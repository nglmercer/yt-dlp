/// Native Likee video page extractor.
pub struct LikeeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LikeeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LikeeExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Likee URL has no video ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let info = json_object_after_marker(&webpage, "window.data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Likee video {video_id} has no window.data payload"),
            )
        })?;
        let video_url = likee_video_url(&info).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Likee video {video_id} has no playable media URL"),
            )
        })?;
        let formats = likee_formats(
            &video_url,
            json_i64(&info, "video_width"),
            json_i64(&info, "video_height"),
        );
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(video_id));
        output.insert_if_some("title", json_string(&info, "msgText"));
        output.insert_if_some("description", json_string(&info, "share_desc"));
        output.insert_if_some("view_count", json_i64(&info, "video_count"));
        output.insert_if_some("like_count", json_i64(&info, "likeCount"));
        output.insert_if_some("comment_count", json_i64(&info, "comment_count"));
        output.insert_if_some("uploader", json_string(&info, "nick_name"));
        output.insert_if_some("uploader_id", json_string(&info, "likeeId"));
        output.insert_if_some(
            "artist",
            info.get("sound")
                .and_then(|sound| json_string(sound, "owner_name")),
        );
        output.insert_if_some(
            "timestamp",
            json_string(&info, "uploadDate").and_then(|value| parse_timestamp(value.to_owned())),
        );
        output.insert_if_some("thumbnail", json_string(&info, "coverUrl"));
        output.insert_if_some(
            "duration",
            info.get("option_data")
                .and_then(|option_data| json_i64(option_data, "dur")),
        );
        output.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(output))
    }
}
