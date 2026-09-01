fn markiza_playlist_items(
    payload: &serde_json::Value,
) -> Option<Vec<&serde_json::Value>> {
    payload
        .get("playlist")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().collect())
}

/// Native Markiza legacy JW-player JSON extractor.
pub struct MarkizaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MarkizaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MarkizaExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Markiza URL has no ID")
            })?;
        let payload = markiza_video_json(context, &video_id)?;
        let details = payload.get("details").unwrap_or(&serde_json::Value::Null);
        let fallback_duration = markiza_duration(details.get("duration"));
        let items = markiza_playlist_items(&payload).unwrap_or_default();
        if items.len() > 1 {
            let mut entries = Vec::new();
            for item in items {
                entries.push(markiza_item_info(item, &video_id, fallback_duration)?);
            }
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(video_id));
            info.insert_if_some("title", markiza_value_string(details.get("name")));
            return Ok(ExtractorResult::Playlist { info, entries });
        }
        let item = items
            .first()
            .copied()
            .unwrap_or(&payload);
        let mut info = markiza_item_info(item, &video_id, fallback_duration)?;
        if info.get_str("title") == Some(video_id.as_str()) {
            info.insert_if_some("title", markiza_value_string(details.get("name")));
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Markiza page-to-legacy-video playlist extractor.
pub struct MarkizaPageExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MarkizaPageExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MarkizaPageExtractor {
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
                    "Markiza page URL has no playlist ID",
                )
            })?;
        let webpage = markiza_page_html(context, url)?;
        let matcher = Regex::new(
            r#"(?is)(?:initPlayer_|data-entity\s*=\s*["']|id\s*=\s*["']player_)(\d+)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Markiza player matcher: {error}"),
            )
        })?;
        let mut seen = Vec::new();
        let mut entries = Vec::new();
        for captures in matcher.captures_iter(&webpage).flatten() {
            let Some(video_id) = captures.get(1).map(|value| value.as_str().to_owned()) else {
                continue;
            };
            if seen.iter().any(|value| value == &video_id) {
                continue;
            }
            seen.push(video_id.clone());
            let mut entry =
                native_url_result(&format!("http://videoarchiv.markiza.sk/video/{video_id}"));
            entry.insert("id", serde_json::json!(video_id));
            entries.push(entry);
        }
        Ok(ExtractorResult::Playlist {
            info: {
                let mut info = InfoDict::new();
                info.insert("id", serde_json::json!(playlist_id));
                info
            },
            entries,
        })
    }
}
