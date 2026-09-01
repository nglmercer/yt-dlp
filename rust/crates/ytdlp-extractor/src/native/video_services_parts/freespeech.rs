/// Native Free Speech story-to-YouTube wrapper.
pub struct FreespeechExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FreespeechExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FreespeechExtractor {
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
                    "Free Speech story URL has no ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let youtube_url = Regex::new(
            r#"(?is)\bdata-video-url\s*=\s*["']([^"']+)["']"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .map(|value| unescape_html_attribute(&value))
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Free Speech story {display_id} has no YouTube URL"),
            )
        })?;
        Ok(ExtractorResult::Redirect {
            url: youtube_url,
            ie_key: Some("Youtube".to_owned()),
        })
    }
}
