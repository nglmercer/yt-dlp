/// Native KTH Play wrapper over the Kaltura API.
pub struct KthExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KthExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KthExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "KTH URL has no video ID")
            })?;
        let target = KalturaTarget {
            partner_id: "308".to_owned(),
            entry_id: video_id,
            player_type: "html5".to_owned(),
            ks: None,
            service_url: "https://api.kaltura.nordu.net".to_owned(),
        };
        kaltura_extract_target(url, context, target)
    }
}
