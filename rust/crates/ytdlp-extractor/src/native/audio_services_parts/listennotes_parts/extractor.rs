/// Native Listen Notes podcast episode extractor.
pub struct ListenNotesExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ListenNotesExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ListenNotesExtractor {
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
        let audio_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Listen Notes URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let mut data = html_script_json(&webpage, "original-content")?;
        let toolbar_attributes = listennotes_toolbar_attributes(&webpage);
        if let Some(object) = data.as_object_mut() {
            object.extend(toolbar_attributes);
        }
        let audio_url = json_string(&data, "audio")
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Listen Notes episode {audio_id} has no audio URL"),
                )
            })?
            .to_owned();
        let title = json_string(&data, "data-title")
            .map(str::to_owned)
            .or_else(|| listennotes_heading(&webpage))
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Listen Notes episode {audio_id} has no title"),
                )
            })?;
        let duration = listennotes_duration(&data).or_else(|| listennotes_meta_duration(&webpage));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("url", serde_json::json!(audio_url));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", listennotes_description(&webpage));
        info.insert_if_some("duration", duration);
        info.insert_if_some(
            "episode_id",
            json_string(&data, "uuid").or_else(|| json_string(&data, "data-episode-uuid")),
        );
        info.insert_if_some("thumbnail", json_string(&data, "data-image"));
        info.insert_if_some("channel", json_string(&data, "data-channel-title"));
        info.insert_if_some("channel_url", json_string(&data, "channel_url"));
        info.insert_if_some("channel_id", json_string(&data, "channel_short_uuid"));
        if let Some(cast) = data
            .get("nlp_entities")
            .and_then(serde_json::Value::as_array)
            .map(|entities| {
                entities
                    .iter()
                    .filter_map(|entity| json_string(entity, "name"))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|cast: &Vec<String>| !cast.is_empty())
        {
            info.insert("cast", serde_json::json!(cast));
        }
        Ok(ExtractorResult::single(info))
    }
}
