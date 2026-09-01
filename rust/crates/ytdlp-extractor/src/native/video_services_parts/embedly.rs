/// Native Embedly widget URL extractor.
pub struct EmbedlyExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EmbedlyExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EmbedlyExtractor {
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
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Embedly widget URL: {error}"),
            )
        })?;
        let query = parsed.query_pairs().collect::<Vec<_>>();
        let source_url = query
        .iter()
        .find(|(key, _)| key == "url")
        .map(|(_, value)| value.clone().into_owned());
        let target_url = query
            .iter()
            .find(|(key, _)| key == "src")
        .or_else(|| query.iter().find(|(key, _)| key == "url"))
            .map(|(_, value)| value.clone().into_owned())
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Embedly widget has no valid src or url target",
                )
            })?;
        if source_url
            .as_deref()
            .is_some_and(embedly_is_youtube_tab_url)
        {
            return Ok(ExtractorResult::Redirect {
                url: source_url.unwrap_or(target_url),
                ie_key: Some("YoutubeTab".to_owned()),
            });
        }
        let mut info = native_url_result(&target_url);
        info.insert("http_headers", serde_json::json!({"Referer": url}));
        Ok(ExtractorResult::single(info))
    }
}

fn embedly_is_youtube_tab_url(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or_default();
    if !host.ends_with("youtube.com") && !host.ends_with("youtu.be") {
        return false;
    }
    parsed.query_pairs().any(|(key, _)| key == "list")
        || parsed
            .path_segments()
            .into_iter()
            .flatten()
            .next()
            .is_some_and(|segment| matches!(segment, "playlist" | "channel" | "user"))
}
