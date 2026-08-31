/// Native VidLii page extractor. Media URLs are embedded in the player
/// configuration and are checked with native HEAD requests before exposure.
pub struct VidLiiExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl VidLiiExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for VidLiiExtractor {
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
                "VidLii URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "VidLii URL has no ID")
            })?;
        let page_url = format!("https://www.vidlii.com/watch?v={video_id}");
        let webpage_response = context.get(&page_url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());
        let parsed_page = url::Url::parse(&page_url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid VidLii page URL: {error}"),
            )
        })?;

        let source_matcher =
            Regex::new(r#"(?is)\bsrc\s*:\s*["']([^"']+)["']"#).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid VidLii source matcher: {error}"),
                )
            })?;
        let height_matcher = Regex::new(r#"(?i)(\d+)\.mp4"#).ok();
        let mut formats = Vec::new();
        for captures in source_matcher.captures_iter(&webpage).flatten() {
            let Some(raw_url) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let source_url = parsed_page
                .join(&proto_relative_url(raw_url, "https:"))
                .map(|value| value.to_string())
                .unwrap_or_else(|_| raw_url.to_owned());
            let height = height_matcher
                .as_ref()
                .and_then(|matcher| matcher.captures(&source_url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<i64>().ok())
                .unwrap_or(360);
            let mut request = Request::new(&source_url);
            request.set_method("HEAD").map_err(map_request_error)?;
            if context.request(&request).is_err() {
                continue;
            }
            formats.push(serde_json::json!({
                "url": source_url,
                "format_id": format!("{height}p"),
                "height": height,
                "ext": "mp4",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("VidLii video {video_id} has no playable source URLs"),
            ));
        }

        let title = Regex::new(r#"(?is)<h1\b[^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                html_title_value(&webpage)
                    .map(|value| value.trim_end_matches(" - VidLii").trim().to_owned())
            })
            .unwrap_or_else(|| video_id.to_owned());
        let description = html_meta_value(&webpage, "description")
            .or_else(|| html_meta_value(&webpage, "twitter:description"))
            .or_else(|| {
                html_element_by_id(&webpage, "des_text")
                    .map(|value| html_text_fragment(&value))
                    .filter(|value| !value.is_empty())
            });
        let thumbnail = html_meta_value(&webpage, "twitter:image").or_else(|| {
            Regex::new(r#"(?is)\bimg\s*:\s*["']([^"']+)["']"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1).map(|value| value.as_str()))
                .and_then(|value| {
                    parsed_page
                        .join(&proto_relative_url(value, "https:"))
                        .ok()
                        .map(|value| value.to_string())
                })
        });
        let (uploader_id, uploader) = Regex::new(
            r#"(?is)<div[^>]*class=["'][^"']*\bwt_person\b[^"']*["'][^>]*>\s*<a[^>]*href=["']/user/([^"'/?#]+)["'][^>]*>(.*?)</a>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .map(|captures| {
            let uploader_id = captures.get(1).map(|value| value.as_str().to_owned());
            let uploader = captures
                .get(2)
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty());
            (uploader_id, uploader)
        })
        .unwrap_or((None, None));
        let upload_date = html_meta_value(&webpage, "datePublished")
            .or_else(|| {
                Regex::new(r#"(?is)<date\b[^>]*>([^<]+)"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                    .and_then(|captures| {
                        captures
                            .get(1)
                            .map(|value| value.as_str().trim().to_owned())
                    })
            })
            .and_then(parse_timestamp);
        let duration = html_meta_value(&webpage, "video:duration")
            .or_else(|| html_json_number(&webpage, "duration"))
            .and_then(|value| value.parse::<f64>().ok());
        let view_count = Regex::new(
            r#"(?is)(?:<strong>\s*([0-9,]+)\s*</strong>\s*views|Views\s*:\s*<strong>\s*([0-9,]+)\s*</strong>)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).or_else(|| captures.get(2)))
        .and_then(|value| value.as_str().replace(',', "").parse::<i64>().ok());
        let comment_count = Regex::new(
            r#"(?is)(?:<span[^>]*id=["']cmt_num["'][^>]*>\s*(\d+)|Comments\s*:\s*<strong>\s*(\d+))"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).or_else(|| captures.get(2)))
        .and_then(|value| value.as_str().parse::<i64>().ok());
        let average_rating = Regex::new(r#"(?is)\brating\s*:\s*([0-9.]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .and_then(|value| value.as_str().parse::<f64>().ok());
        let category =
            Regex::new(r#"(?is)<div>\s*Category\s*:\s*</div>\s*<div>\s*<a[^>]*>([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty());
        let tags =
            Regex::new(r#"(?is)<a[^>]*\bhref=["']/results\?[^"']*\bq=[^"']*["'][^>]*>([^<]+)</a>"#)
                .ok()
                .map(|matcher| {
                    matcher
                        .captures_iter(&webpage)
                        .flatten()
                        .filter_map(|captures| captures.get(1))
                        .map(|value| html_text_fragment(value.as_str()))
                        .filter(|value| !value.is_empty())
                        .map(serde_json::Value::String)
                        .collect::<Vec<_>>()
                })
                .filter(|values| !values.is_empty());
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some(
            "uploader_url",
            uploader_id
                .as_deref()
                .map(|value| format!("https://www.vidlii.com/user/{value}")),
        );
        info.insert_if_some("uploader_id", uploader_id);
        info.insert_if_some("uploader", uploader);
        info.insert_if_some("timestamp", upload_date);
        info.insert_if_some("duration", duration);
        info.insert_if_some("view_count", view_count);
        info.insert_if_some("comment_count", comment_count);
        info.insert_if_some("average_rating", average_rating);
        info.insert_if_some("categories", category.map(|value| vec![value]));
        info.insert_if_some("tags", tags);
        Ok(ExtractorResult::single(info))
    }
}

/// Native PeerTube v1 video API extractor. PeerTube instances share one
/// metadata contract, so the generated URL matcher supplies the instance
/// host and this implementation handles files, streaming playlists, captions,
/// and common account/channel metadata without browser code.
pub struct PeerTubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PeerTubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PeerTubeExtractor {
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
                "PeerTube URL did not match its native pattern",
            )
        })?;
        let host = captures
            .name("host")
            .or_else(|| captures.name("host_2"))
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "PeerTube URL has no host")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "PeerTube URL has no ID")
            })?;
        let api_base = format!("https://{host}/api/v1/videos/{video_id}");
        let video = context.get_json(&api_base)?;
        if let Some(error) = json_string(&video, "error") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("PeerTube API rejected {video_id}: {error}"),
            ));
        }
        let title = json_string(&video, "name").unwrap_or(video_id).to_owned();
        let mut formats = Vec::new();
        let mut is_live = false;
        if let Some(playlists) = video
            .get("streamingPlaylists")
            .and_then(serde_json::Value::as_array)
        {
            for playlist in playlists {
                let Some(playlist_url) = json_string(playlist, "playlistUrl") else {
                    continue;
                };
                is_live = true;
                formats.push(serde_json::json!({
                    "url": playlist_url,
                    "format_id": "hls",
                    "ext": "mp4",
                    "protocol": "m3u8_native",
                }));
                if let Some(playlist_files) =
                    playlist.get("files").and_then(serde_json::Value::as_array)
                {
                    for file in playlist_files {
                        add_peertube_file_format(file, &mut formats);
                    }
                }
            }
        }
        if let Some(files) = video.get("files").and_then(serde_json::Value::as_array) {
            for file in files {
                add_peertube_file_format(file, &mut formats);
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("PeerTube video {video_id} has no playable formats"),
            ));
        }

        let parsed_page = url::Url::parse(&format!("https://{host}")).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid PeerTube host {host}: {error}"),
            )
        })?;
        let webpage_url = format!("https://{host}/videos/watch/{video_id}");
        let thumbnail = json_string(&video, "thumbnailPath")
            .and_then(|path| parsed_page.join(path).ok().map(|value| value.to_string()));
        let description = if json_string(&video, "description")
            .is_some_and(|description| description.len() >= 250)
        {
            context
                .get_json(&format!("{api_base}/description"))
                .ok()
                .and_then(|value| json_string(&value, "description").map(str::to_owned))
                .or_else(|| json_string(&video, "description").map(str::to_owned))
        } else {
            json_string(&video, "description").map(str::to_owned)
        };
        let account = video.get("account").unwrap_or(&serde_json::Value::Null);
        let channel = video.get("channel").unwrap_or(&serde_json::Value::Null);
        let category = video
            .get("category")
            .and_then(|value| json_string(value, "label"))
            .map(|value| vec![serde_json::json!(value)]);
        let subtitles = peertube_subtitles(host, video_id, context);
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some(
            "timestamp",
            json_string(&video, "publishedAt")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("uploader", json_string(account, "displayName"));
        info.insert_if_some(
            "uploader_id",
            json_i64(account, "id").map(|value| value.to_string()),
        );
        info.insert_if_some("uploader_url", json_string(account, "url"));
        info.insert_if_some("channel", json_string(channel, "displayName"));
        info.insert_if_some(
            "channel_id",
            json_i64(channel, "id").map(|value| value.to_string()),
        );
        info.insert_if_some("channel_url", json_string(channel, "url"));
        info.insert_if_some(
            "language",
            video
                .get("language")
                .and_then(|language| json_string(language, "id")),
        );
        info.insert_if_some(
            "license",
            video
                .get("licence")
                .or_else(|| video.get("license"))
                .and_then(|license| json_string(license, "label")),
        );
        info.insert_if_some("duration", json_i64(&video, "duration"));
        info.insert_if_some("view_count", json_i64(&video, "views"));
        info.insert_if_some("like_count", json_i64(&video, "likes"));
        info.insert_if_some("dislike_count", json_i64(&video, "dislikes"));
        info.insert_if_some(
            "age_limit",
            json_bool(&video, "nsfw").map(|value| i64::from(value) * 18),
        );
        info.insert_if_some("tags", video.get("tags").cloned());
        info.insert_if_some("categories", category);
        info.insert_if_some("subtitles", subtitles);
        info.insert("is_live", serde_json::json!(is_live));
        info.insert("webpage_url", serde_json::json!(webpage_url));
        Ok(ExtractorResult::single(info))
    }
}

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

/// Native Rumble page wrapper. Canonical pages embed the same u3 player
/// endpoint used by RumbleEmbedExtractor; page-level counters and description
/// are merged after extracting that native media record.
pub struct RumbleExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RumbleExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RumbleExtractor {
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
        let page_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Rumble page has no ID")
            })?;
        let webpage_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());
        let embed_id = Regex::new(
            r#"(?is)(?:rumble\.com/embed/|["']embedUrl["']\s*:\s*["'](?:https?:)?//rumble\.com/embed/|<iframe[^>]+\bsrc=["'](?:https?:)?//rumble\.com/embed/|Rumble\(\s*["']play["']\s*,\s*\{[^}]*["']?video["']?\s*:\s*["'])([0-9a-z]+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Rumble page {page_id} has no native embed URL"),
            )
        })?;
        let embed_extractor = RumbleEmbedExtractor::new(ExtractorDescriptor::new(
            "RumbleEmbedIE",
            "RumbleEmbed",
            r"https?://(?:www\.)?rumble\.com/embed/(?:[0-9a-z]+\.)?(?P<id>[0-9a-z]+)",
            true,
        ))?;
        let mut info = match embed_extractor
            .extract_with_context(&format!("https://rumble.com/embed/{embed_id}"), context)?
        {
            ExtractorResult::Single(info) => info,
            ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Rumble embed unexpectedly returned a non-media result",
                ));
            }
        };
        info.insert_if_some(
            "release_timestamp",
            Regex::new(
                r#"(?is)(?:Livestream begins|Streamed on):\s*<time[^>]*datetime=["']([^"']+)"#,
            )
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "view_count",
            Regex::new(r#"(?is)"userInteractionCount"\s*:\s*(\d+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<i64>().ok()),
        );
        info.insert_if_some(
            "like_count",
            Regex::new(r#"(?is)<span[^>]*data-js=["']rumbles_up_votes["'][^>]*>\s*([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| parse_compact_count(value.as_str())),
        );
        info.insert_if_some(
            "dislike_count",
            Regex::new(r#"(?is)<span[^>]*data-js=["']rumbles_down_votes["'][^>]*>\s*([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| parse_compact_count(value.as_str())),
        );
        info.insert_if_some(
            "description",
            html_element_by_class(&webpage, "media-description")
                .map(|value| html_text_fragment(&value))
                .filter(|value| !value.is_empty()),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Rumble channel/user listing extractor. Rumble exposes the same
/// video-card markup on both channel and user pages; pagination ends with an
/// empty page or a native HTTP 404, matching the source extractor's behavior.
pub struct RumbleChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RumbleChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RumbleChannelExtractor {
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
                "Rumble channel URL did not match its native pattern",
            )
        })?;
        let base_url = captures
            .name("url")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| url.to_owned());
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Rumble channel has no ID")
            })?;
        let video_extractor = RumbleExtractor::new(ExtractorDescriptor::new(
            "RumbleIE",
            "Rumble",
            r"https?://(?:www\.)?rumble\.com/(?P<id>v[\w.-]+)[^/]*$",
            true,
        ))?;
        let mut entries = Vec::new();
        let mut seen_links = Vec::new();
        for page in 1..=10_000usize {
            let page_url = format!("{base_url}?page={page}");
            let response = match context.get(&page_url) {
                Ok(response) => response,
                Err(error) if error.message.contains("HTTP 404") => break,
                Err(error) => return Err(error),
            };
            let html = String::from_utf8_lossy(response.body());
            let links = rumble_channel_video_links(&html);
            if links.is_empty() {
                break;
            }
            for link in links {
                if seen_links.contains(&link) {
                    continue;
                }
                seen_links.push(link.clone());
                let entry = video_extractor
                    .extract_with_context(&link, context)
                    .map_err(|error| {
                        ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("Rumble channel entry {link}: {error}"),
                        )
                    })?;
                match entry {
                    ExtractorResult::Single(info) => entries.push(info),
                    ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                        return Err(ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("Rumble channel entry {link} returned a non-media result"),
                        ));
                    }
                }
            }
        }
        Ok(ExtractorResult::Playlist {
            info: {
                let mut info = InfoDict::new();
                info.insert("id", serde_json::json!(playlist_id));
                info
            },
            entries,
        })
    }
}

fn rumble_channel_video_links(html: &str) -> Vec<String> {
    let Ok(anchor_matcher) = Regex::new(r"(?is)<a\b([^>]+)>") else {
        return Vec::new();
    };
    let Ok(href_matcher) = Regex::new(r#"(?is)\bhref\s*=\s*[\"']([^\"']+)"#) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    for captures in anchor_matcher.captures_iter(html).flatten() {
        let Some(attributes) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let class_attributes = attributes.to_ascii_lowercase();
        if !class_attributes.contains("videostream__link")
            && !class_attributes.contains("video-item--a")
        {
            continue;
        }
        let Some(href) = href_matcher
            .captures(attributes)
            .ok()
            .flatten()
            .and_then(|value| {
                value
                    .get(1)
                    .map(|value| unescape_html_attribute(value.as_str()))
            })
        else {
            continue;
        };
        let Some(link) = url::Url::parse("https://rumble.com/")
            .ok()
            .and_then(|base| base.join(&href).ok())
            .map(|value| value.to_string())
        else {
            continue;
        };
        if !links.contains(&link) {
            links.push(link);
        }
    }
    links
}

/// Native Slideshare video extractor. The legacy page contains a JSON object
/// assigned to slideshare_object; extracting that object directly avoids a
/// browser or embedded interpreter.
pub struct SlideshareExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SlideshareExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SlideshareExtractor {
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
                "Slideshare URL did not match its native pattern",
            )
        })?;
        let page_title = captures
            .name("title")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| "slideshare".to_owned());
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let payload = json_object_after_marker(&html, "slideshare_object,").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Slideshare page {page_title} has no slideshare_object JSON"),
            )
        })?;
        let slideshow = payload.get("slideshow").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no slideshow metadata",
            )
        })?;
        let slideshow_type = json_string(slideshow, "type").unwrap_or("unknown");
        if slideshow_type != "video" {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Slideshare slideshow type {slideshow_type:?} is not a video"),
            ));
        }
        let player = payload.get("jsplayer").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no jsplayer metadata",
            )
        })?;
        let document = json_string(&payload, "doc").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no document name",
            )
        })?;
        let bucket = json_string(player, "video_bucket").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no video bucket",
            )
        })?;
        let extension = json_string(player, "video_extension").unwrap_or("mp4");
        let bucket_url =
            url::Url::parse(&proto_relative_url(bucket, "https:")).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Slideshare video bucket {bucket:?}: {error}"),
                )
            })?;
        let video_url = bucket_url
            .join(&format!("{document}-SD.{extension}"))
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Slideshare video path: {error}"),
                )
            })?
            .to_string();
        let slideshow_id = json_value_string(slideshow.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare slideshow has no ID",
            )
        })?;
        let title = json_string(slideshow, "title")
            .map(str::to_owned)
            .unwrap_or(page_title);
        let description = html_element_by_id(&html, "slideshow-description-paragraph")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                Regex::new(r#"(?is)<p[^>]*\bitemprop\s*=\s*["']description["'][^>]*>(.*?)</p>"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&html).ok().flatten())
                    .and_then(|captures| captures.get(1))
                    .map(|value| html_text_fragment(value.as_str()))
                    .filter(|value| !value.is_empty())
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(slideshow_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(video_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": video_url,
                "format_id": "sd",
                "ext": extension,
            }]),
        );
        info.insert_if_some("thumbnail", json_string(slideshow, "pin_image_url"));
        info.insert_if_some("description", description);
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Soundgasm single-audio extractor. Audio URLs and metadata are
/// embedded in the page's jPlayer markup and require no JavaScript execution.
pub struct SoundgasmExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SoundgasmExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SoundgasmExtractor {
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
                "Soundgasm URL did not match its native pattern",
            )
        })?;
        let user = captures
            .name("user")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Soundgasm URL has no user")
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Soundgasm URL has no title")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let audio_url = Regex::new(r#"\bm4a\s*:\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Soundgasm audio {display_id} has no m4a URL"),
                )
            })?;
        let title = Regex::new(
            r#"(?is)<div[^>]*\bclass\s*=\s*["'][^"']*\bjp-title\b[^"']*["'][^>]*>(.*?)</div>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| display_id.clone());
        let description = Regex::new(
            r#"(?is)<div[^>]*\bclass\s*=\s*["'][^"']*\bjp-description\b[^"']*["'][^>]*>(.*?)</div>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .or_else(|| {
            Regex::new(r#"(?is)<li>\s*Description:\s*(.*?)</li>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty())
        });
        let audio_id = Regex::new(r#"/([^/]+)\.m4a(?:[?#]|$)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&audio_url).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| display_id.clone());
        let extension = yt_dlp_core::determine_ext(Some(&audio_url), "m4a");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("url", serde_json::json!(audio_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": audio_url,
                "format_id": "audio",
                "ext": extension,
                "vcodec": "none",
            }]),
        );
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert("uploader", serde_json::json!(user));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Soundgasm profile playlist extractor. Profile pages expose links
/// to the same native audio pages, which are expanded in Rust for consistent
/// playlist selection and JSON output.
pub struct SoundgasmProfileExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SoundgasmProfileExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SoundgasmProfileExtractor {
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
                "Soundgasm profile URL did not match its native pattern",
            )
        })?;
        let profile_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Soundgasm profile has no ID",
                )
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let link_matcher =
            Regex::new(r#"(?is)\bhref\s*=\s*["']([^"']+)["']"#).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Soundgasm profile link matcher: {error}"),
                )
            })?;
        let base = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Soundgasm profile URL: {error}"),
            )
        })?;
        let audio_extractor = SoundgasmExtractor::new(ExtractorDescriptor::new(
            "SoundgasmIE",
            "soundgasm",
            r"https?://(?:www\.)?soundgasm\.net/u/(?P<user>[0-9a-zA-Z_-]+)/(?P<display_id>[0-9a-zA-Z_-]+)",
            true,
        ))?;
        let mut entries = Vec::new();
        let mut seen_links = Vec::new();
        for captures in link_matcher.captures_iter(&html).flatten() {
            let Some(raw_link) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let Some(link) = base.join(raw_link).ok().map(|value| value.to_string()) else {
                continue;
            };
            if !link.contains(&format!("/u/{profile_id}/")) || seen_links.contains(&link) {
                continue;
            }
            seen_links.push(link.clone());
            let entry = audio_extractor
                .extract_with_context(&link, context)
                .map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Soundgasm profile entry {link}: {error}"),
                    )
                })?;
            match entry {
                ExtractorResult::Single(info) => entries.push(info),
                ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                    return Err(ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Soundgasm profile entry {link} returned a non-audio result"),
                    ));
                }
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(profile_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
