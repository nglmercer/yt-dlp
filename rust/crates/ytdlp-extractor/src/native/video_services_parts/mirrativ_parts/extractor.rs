pub struct MirrativExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MirrativExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MirrativExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mirrativ URL has no live ID")
            })?;
        let page_url = format!("https://www.mirrativ.com/live/{video_id}");
        let page = context.get(&page_url)?;
        let webpage = String::from_utf8_lossy(page.body());
        let live_data = mirrativ_live_json(context, &video_id)?;
        Ok(ExtractorResult::single(mirrativ_live_info(
            &video_id,
            &webpage,
            &live_data,
        )?))
    }
}

pub struct MirrativUserExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MirrativUserExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MirrativUserExtractor {
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
        let user_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mirrativ URL has no user ID")
            })?;
        let profile = mirrativ_profile_json(context, &user_id)?;
        let mut entries = Vec::new();
        let mut page = 1_i64;
        loop {
            let history = mirrativ_history_json(context, &user_id, page)?;
            if let Some(lives) = history.get("lives").and_then(serde_json::Value::as_array) {
                for live in lives {
                    let is_archive = json_bool(live, "is_archive").unwrap_or(false);
                    let is_live = json_bool(live, "is_live").unwrap_or(false);
                    if !is_archive && !is_live {
                        continue;
                    }
                    let Some(live_id) = mirrativ_value_string(live, "live_id") else {
                        continue;
                    };
                    let mut entry =
                        native_url_result(&format!("https://www.mirrativ.com/live/{live_id}"));
                    entry.insert("ie_key", serde_json::json!("Mirrativ"));
                    entry.insert("id", serde_json::json!(live_id));
                    if let Some(title) = json_string(live, "title") {
                        entry.insert("title", serde_json::json!(title));
                    }
                    entries.push(entry);
                }
            }
            let next_page = json_i64(&history, "next_page");
            let Some(next_page) = next_page.filter(|next_page| *next_page > 0) else {
                break;
            };
            if next_page == page {
                break;
            }
            page = next_page;
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(user_id));
        info.insert_if_some("title", json_string(&profile, "name"));
        info.insert_if_some("description", json_string(&profile, "description"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
