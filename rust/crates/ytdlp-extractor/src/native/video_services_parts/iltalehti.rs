/// Native Iltalehti article wrapper for JWPlatform media.
pub struct IltalehtiExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl IltalehtiExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for IltalehtiExtractor {
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
                    "Iltalehti URL has no article ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let app = json_object_after_marker(&webpage, "window.App").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Iltalehti article {article_id} has no app state"),
            )
        })?;
        let mut video_ids = Vec::new();
        iltalehti_collect_property_media(&app, &mut video_ids);
        video_ids.sort();
        video_ids.dedup();
        if video_ids.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Iltalehti article {article_id} has no JWPlatform media"),
            ));
        }
        let entries = video_ids
            .into_iter()
            .map(|video_id| {
                let mut entry = native_url_result(&format!("jwplatform:{video_id}"));
                entry.insert("ie_key", serde_json::json!("JWPlatform"));
                entry
            })
            .collect();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(article_id));
        info.insert_if_some("title", iltalehti_find_string(&app, "canonical_title"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn iltalehti_collect_property_media(value: &serde_json::Value, video_ids: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(values) => {
            if let Some(properties) = values.get("properties") {
                iltalehti_collect_jw_media(properties, video_ids);
            }
            for value in values.values() {
                iltalehti_collect_property_media(value, video_ids);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                iltalehti_collect_property_media(value, video_ids);
            }
        }
        _ => {}
    }
}

fn iltalehti_collect_jw_media(value: &serde_json::Value, video_ids: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(values) => {
            if json_string(value, "provider")
                .is_some_and(|provider| provider.eq_ignore_ascii_case("jwplayer"))
            {
                if let Some(video_id) = json_value_string(values.get("id"))
                    .filter(|video_id| !video_id.is_empty())
                {
                    video_ids.push(video_id);
                }
            }
            for value in values.values() {
                iltalehti_collect_jw_media(value, video_ids);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                iltalehti_collect_jw_media(value, video_ids);
            }
        }
        _ => {}
    }
}

fn iltalehti_find_string(value: &serde_json::Value, key: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(values) => {
            if let Some(value) = values.get(key).and_then(serde_json::Value::as_str) {
                return Some(value.to_owned());
            }
            values
                .values()
                .find_map(|value| iltalehti_find_string(value, key))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|value| iltalehti_find_string(value, key)),
        _ => None,
    }
}
