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
