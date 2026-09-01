/// Native KelbyOne course playlist extractor.
pub struct KelbyOneExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KelbyOneExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KelbyOneExtractor {
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
        let course_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "KelbyOne URL has no course ID",
                )
            })?;
        let webpage = String::from_utf8_lossy(context.get(url)?.body()).into_owned();
        let playlist_url = kelbyone_playlist_url(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KelbyOne course {course_id} has no JW Platform playlist URL"),
            )
        })?;
        let playlist = context.get_json(&playlist_url)?;
        let items = playlist
            .get("playlist")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("KelbyOne course {course_id} has no playlist entries"),
                )
            })?;
        let entries = items.iter().filter_map(kelbyone_entry).collect::<Vec<_>>();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(course_id));
        info.insert_if_some("title", json_string(&playlist, "title"));
        info.insert_if_some("description", json_string(&playlist, "description"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
