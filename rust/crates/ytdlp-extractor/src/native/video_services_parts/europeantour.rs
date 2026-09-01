/// Native European Tour page-to-Brightcove wrapper.
pub struct EuropeanTourExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EuropeanTourExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EuropeanTourExtractor {
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
        let page_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "European Tour URL has no page ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let captures = Regex::new(
            r#"(?is)brightcove-player\s?video-id\s*=\s*"([^"]+)".*"ACCOUNT_ID"\s*:\s*"([^"]*)""#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("European Tour page {page_id} has no Brightcove player"),
            )
        })?;
        let video_id = captures
            .get(1)
            .map(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("European Tour page {page_id} has no Brightcove video ID"),
                )
            })?;
        let account_id = captures
            .get(2)
            .map(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("5136026580001");
        Ok(ExtractorResult::Redirect {
            url: format!(
                "http://players.brightcove.net/{account_id}/default_default/index.html?videoId={video_id}"
            ),
            ie_key: Some("BrightcoveNew".to_owned()),
        })
    }
}
