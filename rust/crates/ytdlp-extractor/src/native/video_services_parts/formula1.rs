/// Native Formula 1 page-to-Brightcove wrapper.
pub struct Formula1Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Formula1Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Formula1Extractor {
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
        _context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Formula 1 URL has no ID")
            })?;
        Ok(ExtractorResult::Redirect {
            url: format!(
                "http://players.brightcove.net/6057949432001/S1WMrhjlh_default/index.html?videoId={video_id}"
            ),
            ie_key: Some("BrightcoveNew".to_owned()),
        })
    }
}
