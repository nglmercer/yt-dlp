/// Native Kicker page-to-Dailymotion wrapper.
///
/// Kicker delegates playback to Dailymotion in the source extractor. The
/// redirect is represented natively; resolving the target remains an explicit
/// TODO until the Dailymotion GraphQL extractor is ported.
pub struct KickerExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KickerExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KickerExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Kicker URL has no article ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let dailymotion_id =
            Regex::new(r#"(?i)data-dmprivateid\s*=\s*["'](?P<video_id>\w+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.name("video_id"))
                .map(|value| value.as_str().to_owned())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Kicker article {display_id} has no Dailymotion video ID"),
                    )
                })?;
        Ok(ExtractorResult::Redirect {
            url: format!("https://www.dailymotion.com/video/{dailymotion_id}"),
            ie_key: Some("Dailymotion".to_owned()),
        })
    }
}
