pub struct MediaiteExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
    player_matchers: Vec<Regex>,
}

impl MediaiteExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let player_matchers = [
            r#""https://cdn\.jwplayer\.com/players/(?P<id>\w+)"#,
            r#"(?i)\bdata-video-id\s*=\s*["'](?P<id>[^"']+)["']"#,
        ]
        .into_iter()
        .map(|pattern| {
            Regex::new(pattern).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Mediaite player matcher: {error}"),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
            player_matchers,
        })
    }
}

impl InfoExtractor for MediaiteExtractor {
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
        let webpage = mediaite_page(context, url)?;
        let video_id = mediaite_video_id(&webpage, &self.player_matchers)?;
        Ok(ExtractorResult::Redirect {
            url: format!("jwplatform:{video_id}"),
            ie_key: Some("JWPlatform".to_owned()),
        })
    }
}
