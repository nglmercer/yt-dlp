pub struct ManyVidsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ManyVidsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ManyVidsExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "ManyVids URL has no video ID")
            })?;
        let video_data = manyvids_data(context, &video_id, "private")?;
        let (formats, preview_only) = manyvids_formats(&video_data);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ManyVids video {video_id} has no playable media URLs"),
            ));
        }
        let metadata = manyvids_optional_data(context, &video_id);
        let mut output = InfoDict::new();
        let mut output_id = video_id.clone();
        let mut title = metadata
            .as_ref()
            .and_then(|metadata| json_string(metadata, "title"))
            .map(html_text_fragment)
            .filter(|value| !value.is_empty());
        if preview_only {
            output_id.push_str("-preview");
            title = title.map(|value| format!("{value} (Preview)"));
        }
        output.insert("id", serde_json::json!(output_id));
        output.insert_if_some("title", title);
        if let Some(metadata) = metadata {
            output.insert_if_some(
                "description",
                json_string(&metadata, "description")
                    .map(html_text_fragment)
                    .filter(|value| !value.is_empty()),
            );
            output.insert_if_some(
                "uploader",
                metadata
                    .get("model")
                    .and_then(|model| json_string(model, "displayName"))
                    .map(html_text_fragment)
                    .filter(|value| !value.is_empty()),
            );
            output.insert_if_some("thumbnail", manyvids_thumbnail(&metadata));
            output.insert_if_some("view_count", manyvids_count(metadata.get("views")));
            output.insert_if_some("like_count", manyvids_count(metadata.get("likes")));
            output.insert_if_some(
                "release_timestamp",
                json_string(&metadata, "launchDate")
                    .and_then(|value| parse_timestamp(value.to_owned())),
            );
            output.insert_if_some("duration", manyvids_duration(metadata.get("videoDuration")));
            output.insert_if_some("tags", manyvids_tags(&metadata));
        }
        output.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(output))
    }
}
