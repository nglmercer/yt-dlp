/// Native German UPA page-to-Vimeo/Generic redirect extractor.
pub struct GermanupaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
    embed_matcher: Regex,
    login_matcher: Regex,
}

impl GermanupaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let embed_matcher = Regex::new(
            r#"(?is)<iframe[^>]+\bdata-src\s*=\s*['\"](?P<url>https://germanupa\.de/media/oembed\?url=[^'\"]+)['\"]"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid German UPA embed matcher: {error}"),
            )
        })?;
        let login_matcher = Regex::new(r#"(?is)<div[^>]+\bclass\s*=\s*['\"][^'\"]*login-wrapper"#)
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid German UPA login matcher: {error}"),
                )
            })?;
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
            embed_matcher,
            login_matcher,
        })
    }
}

impl InfoExtractor for GermanupaExtractor {
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
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        if let Some(embed_url) = self
            .embed_matcher
            .captures(&webpage)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("url"))
            .map(|value| value.as_str().to_owned())
        {
            let vimeo_url = url_query_value(&embed_url, "url").ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "German UPA oEmbed URL has no nested video URL",
                )
            })?;
            let player_url = vimeo_url
                .strip_prefix("https://vimeo.com/")
                .map_or_else(|| vimeo_url.clone(), |path| format!("https://player.vimeo.com/video/{path}"));
            return Ok(ExtractorResult::Redirect {
                url: player_url,
                ie_key: Some("Vimeo".to_owned()),
            });
        }
        if self.login_matcher.is_match(&webpage).unwrap_or(false) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                "TODO: German UPA member-only media requires authenticated extraction",
            ));
        }
        Ok(ExtractorResult::Redirect {
            url: url.to_owned(),
            ie_key: Some("Generic".to_owned()),
        })
    }
}
