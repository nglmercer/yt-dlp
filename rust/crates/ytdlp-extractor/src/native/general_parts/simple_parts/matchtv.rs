/// Native Match TV live-channel extractor. Both the public on-air URL and
/// the iframe URL share the same channel configuration endpoint.
pub struct MatchTvExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

impl MatchTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let matchers = descriptor
            .valid_urls
            .iter()
            .map(|pattern| {
                compile_source_pattern(pattern).map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("invalid MatchTV URL pattern: {error}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            descriptor,
            matchers,
        })
    }
}

impl InfoExtractor for MatchTvExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    fn extract_with_context(
        &self,
        _url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = "matchtv-live";
        let page_url = "https://video.matchtv.ru/iframe/channel/106";
        let webpage = context.get(page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let matcher = Regex::new(r#"(?is)\bdata-config\s*=\s*"config=(https?://[^?"]+)[?"]"#)
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid MatchTV player matcher: {error}"),
                )
            })?;
        let source_url = matcher
            .captures(&html)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "MatchTV player has no stream configuration URL",
                )
            })?;
        let media_url = format!("{}.m3u8", source_url.replace("/feed/", "/media/"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!("Матч ТВ - Прямой эфир"));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert("is_live", serde_json::json!(true));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
