pub struct MuseScoreExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MuseScoreExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MuseScoreExtractor {
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
        let webpage = musescore_page(context, url)?;
        let canonical_url = musescore_meta(&webpage, "og:url").unwrap_or_else(|| url.to_owned());
        let video_id = self
            .matcher
            .captures(&canonical_url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .or_else(|| {
                self.matcher
                    .captures(url)
                    .ok()
                    .flatten()
                    .and_then(|captures| captures.name("id"))
            })
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "MuseScore URL has no score ID",
                )
            })?;
        let audio_url = musescore_audio_url(context, &video_id)?;
        let formats = musescore_formats(audio_url.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(audio_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("title", musescore_meta(&webpage, "og:title"));
        info.insert_if_some("description", musescore_meta(&webpage, "description"));
        info.insert_if_some("thumbnail", musescore_meta(&webpage, "og:image"));
        info.insert_if_some("uploader", musescore_meta(&webpage, "musescore:author"));
        info.insert_if_some("creator", musescore_meta(&webpage, "musescore:composer"));
        Ok(ExtractorResult::single(info))
    }
}
