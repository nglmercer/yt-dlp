/// Native Uplynk HLS/asset-info extractor.
pub struct UplynkExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl UplynkExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for UplynkExtractor {
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
                "Uplynk URL did not match its native pattern",
            )
        })?;
        let path = captures
            .name("path")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Uplynk URL has no path")
            })?;
        let session_id = captures.name("session_id").map(|value| value.as_str().to_owned());
        Ok(ExtractorResult::single(uplynk_content_info(
            context,
            &path,
            session_id.as_deref(),
            None,
        )?))
    }
}
