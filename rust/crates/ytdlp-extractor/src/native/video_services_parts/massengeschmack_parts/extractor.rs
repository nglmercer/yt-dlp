pub struct MassengeschmackExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MassengeschmackExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MassengeschmackExtractor {
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
        let episode = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Massengeschmack.tv URL has no episode ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let media = massengeschmack_media(&webpage, &episode)?;
        let mut formats = massengeschmack_media_formats(&media);
        formats.extend(massengeschmack_download_formats(&webpage));
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Massengeschmack episode {episode} has no playable media"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(episode));
        info.insert_if_some("title", massengeschmack_title(&webpage));
        info.insert_if_some("thumbnail", massengeschmack_poster(&webpage));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
