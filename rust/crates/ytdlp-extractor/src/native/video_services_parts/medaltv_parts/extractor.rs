pub struct MedalTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MedalTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MedalTvExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Medal.tv URL has no clip ID")
            })?;
        let content = medaltv_content(context, &video_id)?;
        let formats = medaltv_formats(context, &content, &video_id)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&content, "contentTitle"));
        info.insert_if_some("description", json_string(&content, "contentDescription"));
        info.insert_if_some("timestamp", medaltv_timestamp(content.get("created")));
        info.insert_if_some("duration", json_i64(&content, "videoLengthSeconds"));
        info.insert_if_some("view_count", json_i64(&content, "views"));
        info.insert_if_some("like_count", json_i64(&content, "likes"));
        info.insert_if_some("comment_count", json_i64(&content, "comments"));
        info.insert_if_some(
            "uploader",
            content
                .get("poster")
                .and_then(|poster| json_string(poster, "displayName")),
        );
        let uploader_id = medaltv_value_string(
            content
                .get("poster")
                .and_then(|poster| poster.get("userId")),
        );
        info.insert_if_some("uploader_id", uploader_id.clone());
        info.insert_if_some(
            "uploader_url",
            uploader_id.map(|user_id| format!("https://medal.tv/users/{user_id}")),
        );
        info.insert_if_some("thumbnail", medaltv_url(content.get("thumbnailUrl")));
        info.insert_if_some("tags", medaltv_tags(&content));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
