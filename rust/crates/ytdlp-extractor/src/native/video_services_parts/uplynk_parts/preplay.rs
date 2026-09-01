/// Native Uplynk preplay-to-HLS extractor.
pub struct UplynkPreplayExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl UplynkPreplayExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for UplynkPreplayExtractor {
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
                "Uplynk preplay URL did not match its native pattern",
            )
        })?;
        let path = captures
            .name("path")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Uplynk preplay URL has no path",
                )
            })?;
        let preplay = context.get_json(url)?;
        let session_id = json_value_string(preplay.get("sid"));
        Ok(ExtractorResult::single(uplynk_content_info(
            context,
            &path,
            session_id.as_deref(),
            None,
        )?))
    }
}
