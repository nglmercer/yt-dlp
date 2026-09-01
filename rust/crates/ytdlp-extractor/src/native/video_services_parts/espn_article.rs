/// Native ESPN article wrapper that discovers an embedded public clip.
pub struct EspnArticleExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
    espn_clip_matcher: Regex,
    watch_espn_matcher: Regex,
    video_id_matcher: Regex,
}

impl EspnArticleExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let compile = |pattern: &str| {
            Regex::new(pattern).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid ESPN article helper pattern: {error}"),
                )
            })
        };
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
            espn_clip_matcher: compile(
                r####"(?x)
                    https?://
                        (?:
                            (?:
                                (?:
                                    (?:(?:\w+\.)+)?espn\.go|
                                    (?:www\.)?espn
                                )\.com/
                                (?:
                                    (?:
                                        video/(?:clip|iframe/twitter)|
                                    )
                                    (?:
                                        .*?\?.*?\bid=|
                                        /_/id/
                                    )|
                                    [^/]+/video/
                                )
                            )|
                            (?:www\.)espnfc\.(?:com|us)/(?:video/)?[^/]+/\d+/video/
                        )
                        (?P<id>\d+)
                "####,
            )?,
            watch_espn_matcher: compile(
                r"https?://(?:www\.)?espn\.com/(?:watch|espnplus)/player/_/id/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            )?,
            video_id_matcher: compile(
                r####"(?is)class\s*=\s*["'][^"']*video-play-button[^"']*["'][^>]+data-id\s*=\s*["'](?P<id>\d+)"####,
            )?,
        })
    }
}

impl InfoExtractor for EspnArticleExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        if self.espn_clip_matcher.is_match(url).unwrap_or(false)
            || self.watch_espn_matcher.is_match(url).unwrap_or(false)
        {
            return false;
        }
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
        let article_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "ESPN article URL has no article ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let video_id = self
            .video_id_matcher
            .captures(&webpage)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ESPN article {article_id} has no embedded video ID"),
                )
            })?;
        Ok(ExtractorResult::Redirect {
            url: format!("http://espn.go.com/video/clip?id={video_id}"),
            ie_key: Some("ESPN".to_owned()),
        })
    }
}
