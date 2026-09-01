/// Native Fox News canonical video URL wrapper.
pub struct FoxNewsVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FoxNewsVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FoxNewsVideoExtractor {
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
        let video_id = foxnews_match_id(&self.matcher, url, "Fox News video")?;
        Ok(ExtractorResult::Redirect {
            url: format!("https://video.foxnews.com/v/{video_id}"),
            ie_key: Some("FoxNews".to_owned()),
        })
    }
}

/// Native Fox News article-to-video wrapper.
pub struct FoxNewsArticleExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FoxNewsArticleExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FoxNewsArticleExtractor {
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
        let display_id = foxnews_match_id(&self.matcher, url, "Fox News article")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        if let Some(video_id) = Regex::new(
            r#"(?is)\bdata-video-id\s*=\s*["']([^"']+)["']"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        {
            return Ok(ExtractorResult::Redirect {
                url: format!("http://video.foxnews.com/v/{video_id}"),
                ie_key: Some("FoxNews".to_owned()),
            });
        }
        let embed_url = Regex::new(
            r#"(?is)<(?:script|(?:amp-)?iframe)\b[^>]*\bsrc\s*=\s*["']((?:https?:)?//video\.foxnews\.com/v/(?:video-embed\.html|embed\.js)\?(?:[^>"']+&)?(?:video_)?id=\d+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str()))
        .map(|value| proto_relative_url(value, "https:"));
        embed_url
            .map(|url| ExtractorResult::Redirect {
                url,
                ie_key: Some("FoxNews".to_owned()),
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Fox News article {display_id} has no video embed"),
                )
            })
    }
}
