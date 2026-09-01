/// Native Last.fm track-to-YouTube redirect extractor.
pub struct LastFmExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LastFmExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LastFmExtractor {
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
        let track_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Last.fm track URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let player_url = Regex::new(
            r#"(?is)class\s*=\s*["']header-new-playlink["']\s+href\s*=\s*["']([^"']+)["']"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| resolve_url(url, value.as_str()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Last.fm track {track_id} has no YouTube player URL"),
            )
        })?;
        Ok(ExtractorResult::Redirect {
            url: player_url,
            ie_key: Some("Youtube".to_owned()),
        })
    }
}
