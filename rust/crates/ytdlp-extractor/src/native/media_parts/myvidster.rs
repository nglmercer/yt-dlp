/// Native MyVidster page extractor.
///
/// MyVidster keeps the playable resource in a `videolink` anchor. Returning
/// an explicit native redirect lets the Rust CLI continue through the normal
/// extractor registry without a compatibility runtime.
pub struct MyVidsterExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MyVidsterExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MyVidsterExtractor {
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
                "MyVidster URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "MyVidster URL has no ID")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let target = Regex::new(
            r#"(?is)\brel\s*=\s*["']videolink["'][^>]*\bhref\s*=\s*["']([^"']+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
        .filter(|value| !value.trim().is_empty())
        .map(|value| resolve_url(url, value.trim()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MyVidster page {video_id} has no videolink target"),
            )
        })?;

        Ok(ExtractorResult::Redirect {
            url: target,
            ie_key: None,
        })
    }
}
