/// Native Jamendo album playlist extractor.
pub struct JamendoAlbumExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JamendoAlbumExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JamendoAlbumExtractor {
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
        let album_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Jamendo album has no ID")
            })?;
        let album = jamendo_call_api(context, "album", &album_id)?;
        let album_name = jamendo_string(album.get("name"));
        let entries = album
            .get("tracks")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|track| json_value_string(track.get("id")))
            .map(|track_id| {
                let mut entry = native_url_result(&format!("https://www.jamendo.com/track/{track_id}"));
                entry.insert("_type", serde_json::json!("url_transparent"));
                entry.insert("ie_key", serde_json::json!("Jamendo"));
                entry.insert("id", serde_json::json!(track_id));
                entry.insert_if_some("album", album_name.clone());
                entry
            })
            .collect();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(album_id));
        info.insert_if_some("title", album_name);
        info.insert_if_some(
            "description",
            album
                .get("description")
                .and_then(|description| description.get("en"))
                .and_then(serde_json::Value::as_str)
                .map(html_text_fragment),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
