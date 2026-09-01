pub struct MdrExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MdrExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MdrExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "MDR URL has no video ID")
            })?;
        let webpage = mdr_page(context, url)?;
        let data_url = mdr_data_url(&webpage)?;
        let xml_url = resolve_url(url, &data_url);
        let xml_response = context.get(&xml_url)?;
        let document = mdr_parse_xml(xml_response.body())?;
        let title = document
            .title
            .as_deref()
            .or(document.broadcast_name.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("MDR video {video_id} has no title"),
                )
            })?;
        let formats = mdr_formats(&document, &video_id)?;
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let timestamp = [
            document.broadcast_date.as_deref(),
            document.broadcast_start_date.as_deref(),
            document.broadcast_end_date.as_deref(),
        ]
        .into_iter()
        .flatten()
        .find_map(|value| yt_dlp_core::parse_iso8601(value));
        let description = document
            .description
            .as_deref()
            .or(document.broadcast_description.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some(
            "duration",
            document
                .duration
                .as_deref()
                .and_then(yt_dlp_core::parse_duration),
        );
        info.insert_if_some(
            "uploader",
            document
                .uploader
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
