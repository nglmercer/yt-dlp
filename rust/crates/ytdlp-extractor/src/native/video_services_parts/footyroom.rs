/// Native FootyRoom match playlist extractor.
pub struct FootyRoomExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FootyRoomExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FootyRoomExtractor {
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
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "FootyRoom URL has no match ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let media = json_array_after_marker(&webpage, "DataStore.media").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FootyRoom match {playlist_id} has no media data"),
            )
        })?;
        let streamable_matcher = Regex::new(
            r#"(?is)<iframe\b[^>]*\bsrc\s*=\s*["']((?:https?:)?//streamable\.com/[^"']+)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid FootyRoom Streamable matcher: {error}"),
            )
        })?;
        let mut entries = Vec::new();
        for video in media.as_array().into_iter().flatten() {
            let Some(payload) = json_string(video, "payload") else {
                continue;
            };
            let streamable_url = streamable_matcher
                .captures(payload)
                .ok()
                .flatten()
                .and_then(|captures| captures.get(1))
                .map(|value| proto_relative_url(value.as_str(), "https:"))
                .or_else(|| {
                    payload
                        .trim()
                        .strip_prefix("https://streamable.com/")
                        .map(|_| payload.trim().to_owned())
                });
            let Some(streamable_url) = streamable_url else {
                continue;
            };
            let mut entry = native_url_result(&streamable_url);
            entry.insert("ie_key", serde_json::json!("Streamable"));
            entries.push(entry);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FootyRoom match {playlist_id} has no Streamable entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some(
            "title",
            html_meta_value(&webpage, "og:title")
                .map(|value| html_text_fragment(&value))
                .filter(|value| !value.is_empty()),
        );
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
