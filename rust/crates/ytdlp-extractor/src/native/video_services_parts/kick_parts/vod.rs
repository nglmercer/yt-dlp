/// Native Kick VOD extractor.
pub struct KickVodExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KickVodExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KickVodExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Kick VOD URL has no video ID")
            })?;
        let response = kick_api_json(context, &format!("v1/video/{video_id}"))?;
        let source_url = json_string(&response, "source")
            .and_then(kick_valid_url)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kick VOD {video_id} has no source URL"),
                )
            })?;
        let formats = vec![kick_media_format(&source_url, "hls")];
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let livestream = response.get("livestream").unwrap_or(&serde_json::Value::Null);
        let channel = livestream.get("channel").unwrap_or(&serde_json::Value::Null);
        let title = json_string(livestream, "session_title")
            .or_else(|| json_string(livestream, "slug"))
            .map(str::to_owned);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
        info.insert_if_some(
            "description",
            channel
                .get("user")
                .and_then(|user| json_string(user, "bio")),
        );
        info.insert_if_some("channel", json_string(channel, "slug"));
        info.insert_if_some("channel_id", json_value_string(channel.get("id")));
        info.insert_if_some(
            "uploader",
            channel
                .get("user")
                .and_then(|user| json_string(user, "username")),
        );
        info.insert_if_some(
            "uploader_id",
            json_value_string(channel.get("user_id"))
                .or_else(|| channel.get("user").and_then(|user| json_value_string(user.get("id")))),
        );
        info.insert_if_some(
            "timestamp",
            kick_timestamp(json_string(&response, "created_at")),
        );
        info.insert_if_some(
            "upload_date",
            json_string(&response, "created_at").and_then(date_digits),
        );
        info.insert_if_some(
            "duration",
            json_f64(livestream, "duration").map(|value| value / 1000.0),
        );
        info.insert_if_some("thumbnail", kick_optional_url(livestream.get("thumbnail")));
        info.insert_if_some(
            "categories",
            kick_category_names(livestream.get("categories")),
        );
        info.insert_if_some("view_count", json_i64(&response, "views"));
        info.insert_if_some(
            "age_limit",
            kick_age_limit(livestream.get("is_mature")),
        );
        info.insert_if_some("is_live", json_bool(livestream, "is_live"));
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
