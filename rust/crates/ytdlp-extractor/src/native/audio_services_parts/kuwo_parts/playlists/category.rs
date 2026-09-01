/// Native Kuwo category playlist extractor.
pub struct KuwoCategoryExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KuwoCategoryExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KuwoCategoryExtractor {
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
        let category_id = kuwo_match_id(&self.matcher, url, "category")?;
        let (webpage, _) = kuwo_page(context, url, "category detail")?;
        let matcher = Regex::new(
            r#"(?is)<h1[^>]+title\s*=\s*["']([^<>]+?)["'][^>]*>[^<]*</h1>"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Kuwo category title matcher: {error}"),
            )
        })?;
        let category_name = matcher
            .captures(&webpage)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kuwo category {category_id} has no category name"),
                )
            })?;
        let jsonm = json_object_after_marker(&webpage, "var jsonm").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kuwo category {category_id} has no song data"),
            )
        })?;
        let entries = jsonm
            .get("musiclist")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|song| json_value_string(song.get("musicrid")))
            .map(|song_id| kuwo_entry(&kuwo_song_url(&song_id)))
            .collect::<Vec<_>>();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(category_id));
        info.insert("title", serde_json::json!(category_name.clone()));
        info.insert_if_some("description", kuwo_intro(&webpage, &category_name));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
