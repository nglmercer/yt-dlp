/// Native Gronkh discovery-feed playlist extractor.
pub struct GronkhFeedExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GronkhFeedExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GronkhFeedExtractor {
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
        _url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let mut entries = Vec::new();
        for discovery_type in ["recent", "views"] {
            let data = context.get_json(&format!(
                "https://api.gronkh.tv/v1/video/discovery/{discovery_type}"
            ))?;
            let Some(items) = data.get("discovery").and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for item in items {
                entries.push(gronkh_index_entry(item, true)?);
            }
        }

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!("feed"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Gronkh VOD search/index playlist extractor.
pub struct GronkhVodsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GronkhVodsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GronkhVodsExtractor {
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
        _url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        const PER_PAGE: usize = 25;
        const MAX_PAGES: usize = 10_000;

        let mut entries = Vec::new();
        for page in 0..MAX_PAGES {
            let data = context.get_json(&format!(
                "https://api.gronkh.tv/v1/search?offset={}&first={PER_PAGE}",
                PER_PAGE * page
            ))?;
            let Some(items) = data
                .get("results")
                .and_then(|results| results.get("videos"))
                .and_then(serde_json::Value::as_array)
            else {
                break;
            };
            if items.is_empty() {
                break;
            }
            for item in items {
                entries.push(gronkh_index_entry(item, false)?);
            }
        }

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!("vods"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn gronkh_index_entry(
    item: &serde_json::Value,
    feed_entry: bool,
) -> Result<InfoDict, ExtractorError> {
    let episode = json_value_string(item.get("episode")).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Gronkh index item has no episode identifier",
        )
    })?;
    let mut entry = native_url_result(&format!(
        "https://gronkh.tv/watch/stream/{episode}"
    ));
    entry.insert("ie_key", serde_json::json!("Gronkh"));
    if feed_entry {
        entry.insert_if_some("id", json_value_string(item.get("title")));
    } else {
        entry.insert("id", serde_json::json!(episode));
        entry.insert_if_some("title", json_value_string(item.get("title")));
    }
    Ok(entry)
}
