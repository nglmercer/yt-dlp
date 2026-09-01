/// Native HGTV show playlist configuration extractor.
pub struct HgtvComShowExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HgtvComShowExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HgtvComShowExtractor {
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
        let playlist_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "HGTV URL has no ID"))?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let config = json_object_after_marker(&webpage, "text/x-config").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HGTV show {playlist_id} has no video configuration"),
            )
        })?;
        let channel = config
            .get("channels")
            .and_then(serde_json::Value::as_array)
            .and_then(|channels| channels.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("HGTV show {playlist_id} has no channel configuration"),
                )
            })?;
        let mut entries = Vec::new();
        if let Some(videos) = channel.get("videos").and_then(serde_json::Value::as_array) {
            for video in videos {
                let Some(release_url) = json_string(video, "releaseUrl")
                    .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
                else {
                    continue;
                };
                entries.push(native_url_result(release_url));
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", json_string(channel, "title"));
        info.insert_if_some("description", json_string(channel, "description"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
