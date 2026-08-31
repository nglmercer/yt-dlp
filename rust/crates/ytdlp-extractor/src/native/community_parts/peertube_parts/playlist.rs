/// Native PeerTube account/channel/playlist extractor. The API is paginated
/// with stable offsets; entries are expanded through the native video
/// extractor so playlist downloads never need a Python callback or URL-result
/// compatibility layer.
pub struct PeerTubePlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PeerTubePlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PeerTubePlaylistExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matcher.is_match(url).unwrap_or(false)
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        1
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "PeerTube playlist URL did not match its native pattern",
            )
        })?;
        let host = captures
            .name("host")
            .or_else(|| captures.name("host_2"))
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "PeerTube URL has no host")
            })?;
        let playlist_type = captures
            .name("type")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "PeerTube playlist URL has no resource type",
                )
            })?;
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "PeerTube playlist URL has no resource ID",
                )
            })?;
        let api_resource = match playlist_type {
            "a" => "accounts",
            "c" => "video-channels",
            "w/p" => "video-playlists",
            _ => {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!("TODO: unsupported PeerTube resource type {playlist_type}"),
                ));
            }
        };
        let api_base = format!("https://{host}/api/v1/{api_resource}/{playlist_id}");
        let playlist = context.get_json(&api_base)?;
        if let Some(error) = json_string(&playlist, "error") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("PeerTube API rejected {playlist_id}: {error}"),
            ));
        }

        let video_extractor = PeerTubeExtractor::new(ExtractorDescriptor::new(
            "PeerTubeIE",
            "PeerTube",
            r"https?://(?P<host>[^/]+)/w/(?P<id>[^/?#]+)",
            true,
        ))?;
        const PAGE_SIZE: usize = 30;
        let mut entries = Vec::new();
        for page in 0usize.. {
            let start = page.checked_mul(PAGE_SIZE).ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    "TODO: PeerTube playlist pagination exceeded native bounds",
                )
            })?;
            let response = context.get_json(&format!(
                "{api_base}/videos?sort=-createdAt&start={start}&count={PAGE_SIZE}&nsfw=both"
            ))?;
            let data = response
                .get("data")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let page_len = data.len();
            for video in data {
                let short_uuid = json_string(&video, "shortUUID").or_else(|| {
                    video
                        .get("video")
                        .and_then(|nested| json_string(nested, "shortUUID"))
                });
                let Some(short_uuid) = short_uuid else {
                    continue;
                };
                let entry_url = format!("https://{host}/w/{short_uuid}");
                let entry = video_extractor
                    .extract_with_context(&entry_url, context)
                    .map_err(|error| {
                        ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("PeerTube playlist entry {short_uuid}: {error}"),
                        )
                    })?;
                match entry {
                    ExtractorResult::Single(info) => entries.push(info),
                    ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                        return Err(ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("PeerTube entry {short_uuid} returned a non-media result"),
                        ));
                    }
                }
            }
            if page_len < PAGE_SIZE {
                break;
            }
        }

        let thumbnail = json_string(&playlist, "thumbnailPath").and_then(|path| {
            url::Url::parse(&format!("https://{host}"))
                .ok()?
                .join(path)
                .ok()
                .map(|value| value.to_string())
        });
        let owner = playlist
            .get("ownerAccount")
            .or_else(|| playlist.get("owner"))
            .unwrap_or(&serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some(
            "title",
            json_string(&playlist, "displayName").or_else(|| json_string(&playlist, "name")),
        );
        info.insert_if_some("description", json_string(&playlist, "description"));
        info.insert_if_some(
            "timestamp",
            json_string(&playlist, "createdAt")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "channel",
            json_string(owner, "name").or_else(|| json_string(&playlist, "displayName")),
        );
        info.insert_if_some(
            "channel_id",
            json_value_string(owner.get("id").or_else(|| playlist.get("id"))),
        );
        info.insert_if_some("thumbnail", thumbnail);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
