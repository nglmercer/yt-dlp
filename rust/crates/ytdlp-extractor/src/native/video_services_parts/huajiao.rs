/// Native Huajiao archived-live HLS extractor.
pub struct HuajiaoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HuajiaoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HuajiaoExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Huajiao URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let feed = json_object_after_marker(&webpage, "var feed").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Huajiao page {video_id} has no feed JSON"),
            )
        })?;
        let feed_data = feed.get("feed").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Huajiao item {video_id} has no feed data"),
            )
        })?;
        let title = json_string(feed_data, "formated_title").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Huajiao item {video_id} has no title"),
            )
        })?;
        let media_url = json_string(feed_data, "m3u8")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Huajiao item {video_id} has no HLS URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", html_meta_value(&webpage, "description"));
        info.insert_if_some("duration", huajiao_duration(feed_data));
        info.insert_if_some("thumbnail", json_string(feed_data, "image"));
        info.insert_if_some(
            "timestamp",
            json_string(&feed, "creatime").and_then(huajiao_timestamp),
        );
        if let Some(author) = feed.get("author") {
            info.insert_if_some("uploader", json_string(author, "nickname"));
            info.insert_if_some("uploader_id", json_value_string(author.get("uid")));
        }
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn huajiao_duration(feed: &serde_json::Value) -> Option<f64> {
    json_f64(feed, "duration").or_else(|| {
        json_string(feed, "duration").and_then(|value| yt_dlp_core::parse_duration(value.trim()))
    })
}

fn huajiao_timestamp(value: &str) -> Option<i64> {
    parse_timestamp(value.to_owned()).or_else(|| parse_timestamp(value.replace(' ', "T")))
}
