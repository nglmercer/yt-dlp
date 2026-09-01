pub struct MuseAiExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MuseAiExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MuseAiExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "MuseAI URL has no video ID")
            })?;
        let webpage = museai_page(context, &video_id)?;
        let data = museai_player_data(&webpage, &video_id)?;
        let formats = museai_formats(&data, &video_id)?;
        let first_format = formats.first().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MuseAI video {video_id} has no first format"),
            )
        })?;
        let first_url = first_format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("MuseAI video {video_id} has an invalid source format"),
                )
            })?;
        let first_ext = first_format
            .get("ext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!(first_ext));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert_if_some("description", json_string(&data, "description"));
        info.insert_if_some("duration", json_f64(&data, "duration"));
        info.insert_if_some("timestamp", json_i64(&data, "tcreated"));
        info.insert_if_some("uploader", json_string(&data, "owner_name"));
        info.insert_if_some("uploader_id", json_string(&data, "owner_username"));
        info.insert_if_some("view_count", json_i64(&data, "views"));
        info.insert_if_some(
            "age_limit",
            json_bool(&data, "mature")
                .filter(|mature| *mature)
                .map(|_| 18),
        );
        info.insert_if_some(
            "availability",
            json_string(&data, "visibility").map(|visibility| {
                if matches!(visibility, "private" | "unlisted") {
                    visibility.to_owned()
                } else {
                    "public".to_owned()
                }
            }),
        );
        Ok(ExtractorResult::single(info))
    }
}
