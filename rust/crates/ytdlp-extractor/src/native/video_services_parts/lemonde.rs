/// Native Lemonde article-to-provider redirect extractor.
pub struct LemondeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LemondeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LemondeExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Lemonde URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let digiteka_url = Regex::new(
            r#"(?is)\burl\s*:\s*["'](?P<url>(?:https?:)?//(?:www\.)?(?:digiteka\.net|ultimedia\.com)/deliver/.+?)["']"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.name("url"))
        .map(|value| proto_relative_url(value.as_str(), "https:"));
        if let Some(digiteka_url) = digiteka_url {
            return Ok(ExtractorResult::Redirect {
                url: digiteka_url,
                ie_key: Some("Digiteka".to_owned()),
            });
        }
        // This mirrors source Generic fallback; GenericIE is native and does
        // not require a compatibility/runtime bridge.
        if display_id.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Lemonde URL has an empty display ID",
            ));
        }
        Ok(ExtractorResult::Redirect {
            url: url.to_owned(),
            ie_key: Some("Generic".to_owned()),
        })
    }
}
