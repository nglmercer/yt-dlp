/// Native FOX 9 video URL wrapper.
pub struct Fox9Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Fox9Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Fox9Extractor {
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
        let video_id = fox9_match_id(&self.matcher, url, "FOX 9 video")?;
        Ok(ExtractorResult::Redirect {
            url: format!(
                "anvato:anvato_epfox_app_web_prod_b3373168e12f423f41504f207000188daf88251b:{video_id}"
            ),
            ie_key: Some("Anvato".to_owned()),
        })
    }
}

/// Native FOX 9 article-to-video wrapper.
pub struct Fox9NewsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Fox9NewsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Fox9NewsExtractor {
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
        let display_id = fox9_match_id(&self.matcher, url, "FOX 9 article")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let video_id = Regex::new(r#"(?is)\banvatoId\s*:\s*[\"'](\d+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FOX 9 article {display_id} has no Anvato video ID"),
                )
            })?;
        Ok(ExtractorResult::Redirect {
            url: format!("https://www.fox9.com/video/{video_id}"),
            ie_key: Some("FOX9".to_owned()),
        })
    }
}

fn fox9_match_id(
    matcher: &Regex,
    url: &str,
    label: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, format!("{label} URL has no ID")))
}
