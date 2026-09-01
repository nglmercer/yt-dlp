/// Native MeWatch item API redirect into the Toggle extractor.
pub struct MeWatchExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MeWatchExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MeWatchExtractor {
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
        let item_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "MeWatch URL has no item ID")
            })?;
        let custom_id = mewatch_custom_id(context, &item_id)?;
        Ok(ExtractorResult::Redirect {
            url: format!("toggle:{custom_id}"),
            ie_key: Some("Toggle".to_owned()),
        })
    }
}
