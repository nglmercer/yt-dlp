/// Native Toggle media-information API extractor.
pub struct ToggleExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ToggleExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ToggleExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Toggle URL has no media ID")
            })?;
        let api_info = toggle_media_info(context, &video_id)?;
        let title = json_string(&api_info, "MediaName")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Toggle media {video_id} has no title"),
                )
            })?;
        let formats = toggle_formats(&api_info, &video_id)?;
        let thumbnails = toggle_thumbnails(&api_info);
        let timestamp = json_string(&api_info, "CreationDate")
            .map(str::to_owned)
            .and_then(parse_timestamp);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            json_string(&api_info, "Description")
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some("duration", json_i64(&api_info, "Duration"));
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some(
            "upload_date",
            json_string(&api_info, "CreationDate").and_then(date_digits),
        );
        info.insert_if_some("average_rating", json_f64(&api_info, "Rating"));
        info.insert_if_some("view_count", toggle_counter(&api_info, "View"));
        info.insert_if_some("like_count", toggle_counter(&api_info, "Like"));
        info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        info.insert("formats", serde_json::Value::Array(formats.clone()));
        if let Some(first) = formats.first() {
            info.insert_if_some("url", first.get("url").cloned());
            info.insert_if_some("ext", first.get("ext").cloned());
        }
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
