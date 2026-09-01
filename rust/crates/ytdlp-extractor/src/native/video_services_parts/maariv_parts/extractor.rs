pub struct MaarivExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MaarivExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MaarivExtractor {
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
                "Maariv player URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Maariv player URL has no media ID")
            })?;
        let data = maariv_media_data(context, &video_id)?;
        let formats = maariv_formats(&data, &video_id)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert_if_some(
            "duration",
            data.get("video").and_then(|video| json_i64(video, "duration")),
        );
        info.insert_if_some(
            "timestamp",
            json_string(&data, "upload_date")
                .and_then(|value| parse_timestamp(value.to_owned())),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
