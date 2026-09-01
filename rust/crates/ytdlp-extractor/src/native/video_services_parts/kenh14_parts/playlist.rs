/// Native Kenh14 playlist page extractor.
pub struct Kenh14PlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Kenh14PlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Kenh14PlaylistExtractor {
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
        let playlist_id = kenh14_match_id(&self.matcher, url, "playlist")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let json_ld = html_json_ld(&webpage).unwrap_or(serde_json::Value::Null);
        let title = html_element_by_class(&webpage, "name")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| json_string(&json_ld, "name").map(str::to_owned));
        let description = html_element_by_class(&webpage, "description")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| json_string(&json_ld, "alternateName").map(str::to_owned));
        let thumbnail =
            html_meta_value(&webpage, "og:image").map(|value| kenh14_remove_query(&value));
        let Some(matcher) = Regex::new(
            r#"(?is)<[^>]*\bclass\s*=\s*["'][^"']*\bvideo-item\b[^"']*["'][^>]*>"#,
        )
        .ok()
        else {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kenh14 playlist {playlist_id} has invalid item matcher"),
            ));
        };
        let mut entries = Vec::new();
        for captures in matcher.captures_iter(&webpage).flatten() {
            let Some(item_tag) = captures.get(0).map(|value| value.as_str()) else {
                continue;
            };
            let Some(video_id) = kenh14_attribute(item_tag, "data-id") else {
                continue;
            };
            let mut entry = native_url_result(&format!(
                "https://video.kenh14.vn/video/video-{video_id}.chn"
            ));
            entry.insert("ie_key", serde_json::json!("Kenh14Video"));
            entry.insert("id", serde_json::json!(video_id));
            entries.push(entry);
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn kenh14_remove_query(value: &str) -> String {
    url::Url::parse(value)
        .map(|mut value| {
            value.set_query(None);
            value.set_fragment(None);
            value.to_string()
        })
        .unwrap_or_else(|_| value.to_owned())
}
