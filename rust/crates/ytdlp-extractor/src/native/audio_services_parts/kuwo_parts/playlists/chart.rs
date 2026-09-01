/// Native Kuwo chart playlist extractor.
pub struct KuwoChartExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KuwoChartExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KuwoChartExtractor {
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
        let chart_id = kuwo_match_id(&self.matcher, url, "chart")?;
        let (webpage, _) = kuwo_page(context, url, "chart detail")?;
        let link_matcher =
            Regex::new(r#"(?is)<a[^>]+href\s*=\s*["']http://www\.kuwo\.cn/yinyue/(\d+)"#).map_err(
                |error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("invalid Kuwo chart entry matcher: {error}"),
                    )
                },
            )?;
        let entries = link_matcher
            .captures_iter(&webpage)
            .flatten()
            .filter_map(|captures| captures.get(1))
            .map(|value| kuwo_entry(&kuwo_song_url(value.as_str())))
            .collect::<Vec<_>>();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(chart_id));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
