/// Native Kick live-channel extractor.
pub struct KickExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KickExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KickExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matcher.is_match(url).unwrap_or(false)
            && !kick_is_vod_url(url)
            && !kick_is_clip_url(url)
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
        let channel = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Kick URL has no channel")
            })?;
        let response = kick_api_json(context, &format!("v2/channels/{channel}"))?;
        let livestream = response.get("livestream").filter(|value| !value.is_null()).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kick channel {channel} is not live"),
            )
        })?;
        let playback_url = json_string(&response, "playback_url")
            .and_then(kick_valid_url)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kick channel {channel} has no playback URL"),
                )
            })?;
        let formats = vec![kick_media_format(&playback_url, "hls")];
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let title = json_string(livestream, "session_title")
            .map(str::to_owned)
            .unwrap_or_else(|| channel.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(json_string(livestream, "slug").unwrap_or(channel.as_str())));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            response
                .get("user")
                .and_then(|user| json_string(user, "bio"))
                .map(str::to_owned),
        );
        info.insert("channel", serde_json::json!(channel));
        info.insert_if_some(
            "channel_id",
            json_value_string(response.get("id"))
                .or_else(|| json_value_string(livestream.get("channel_id"))),
        );
        info.insert_if_some(
            "uploader",
            json_string(&response, "name")
                .or_else(|| response.get("user").and_then(|user| json_string(user, "username"))),
        );
        info.insert_if_some(
            "uploader_id",
            json_value_string(response.get("user_id"))
                .or_else(|| response.get("user").and_then(|user| json_value_string(user.get("id")))),
        );
        info.insert_if_some(
            "timestamp",
            kick_timestamp(json_string(livestream, "created_at")),
        );
        info.insert_if_some(
            "upload_date",
            json_string(livestream, "created_at").and_then(date_digits),
        );
        info.insert_if_some(
            "release_timestamp",
            kick_timestamp(json_string(livestream, "start_time")),
        );
        info.insert_if_some(
            "release_date",
            json_string(livestream, "start_time").and_then(date_digits),
        );
        info.insert_if_some(
            "thumbnail",
            kick_optional_url(livestream.get("thumbnail")),
        );
        info.insert_if_some(
            "categories",
            kick_category_names(response.get("recent_categories")),
        );
        info.insert_if_some(
            "concurrent_view_count",
            json_i64(livestream, "viewer_count"),
        );
        info.insert_if_some(
            "age_limit",
            kick_age_limit(livestream.get("is_mature")),
        );
        info.insert("is_live", serde_json::json!(true));
        info.insert(
            "url",
            first
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
