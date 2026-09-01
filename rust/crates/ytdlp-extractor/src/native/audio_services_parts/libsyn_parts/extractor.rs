/// Native Libsyn HTML5 audio embed extractor.
pub struct LibsynExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LibsynExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LibsynExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Libsyn URL has no episode ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let data = json_object_after_marker(&webpage, "var playlistItem").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Libsyn episode {video_id} has no playlistItem data"),
            )
        })?;
        let episode_title = libsyn_episode_title(&data, &webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Libsyn episode {video_id} has no title"),
            )
        })?;
        let title = libsyn_podcast_title(&webpage)
            .map(|podcast_title| format!("{podcast_title} - {episode_title}"))
            .unwrap_or(episode_title);
        let formats = libsyn_formats(&data);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Libsyn episode {video_id} has no media URLs"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", libsyn_description(&webpage));
        info.insert_if_some("thumbnail", json_string(&data, "thumbnail_url"));
        info.insert_if_some("upload_date", libsyn_release_date(&data, &webpage));
        info.insert_if_some("duration", libsyn_duration(data.get("duration")));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
