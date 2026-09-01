/// Native Hytale article-to-Cloudflare Stream playlist extractor.
pub struct HytaleExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
    stream_matcher: Regex,
}

impl HytaleExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let stream_matcher = Regex::new(
            r#"(?is)<stream\s+class\s*=\s*[\"']ql-video\s+cf-stream[\"']\s+src\s*=\s*[\"']([a-f0-9]{32})[\"']"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Hytale stream matcher: {error}"),
            )
        })?;
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
            stream_matcher,
        })
    }
}

impl InfoExtractor for HytaleExtractor {
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
        let playlist_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Hytale URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let titles = context
            .get("https://hytale.com/media")
            .ok()
            .map(|response| hytale_titles(&String::from_utf8_lossy(response.body())))
            .unwrap_or_default();
        let mut entries = Vec::new();
        for captures in self.stream_matcher.captures_iter(&webpage).flatten() {
            let Some(video_hash) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let mut entry = native_url_result(&format!(
                "https://cloudflarestream.com/{video_hash}/manifest/video.mpd?parentOrigin=https%3A%2F%2Fhytale.com"
            ));
            entry.insert("_type", serde_json::json!("url_transparent"));
            entry.insert("ie_key", serde_json::json!("CloudflareStream"));
            entry.insert_if_some("title", titles.get(video_hash).cloned());
            entries.push(entry);
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some(
            "title",
            html_meta_value(&webpage, "og:title").or_else(|| html_title_value(&webpage)),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn hytale_titles(html: &str) -> std::collections::HashMap<String, String> {
    let Some(state) = json_array_after_marker(html, "window.__INITIAL_COMPONENTS_STATE__") else {
        return std::collections::HashMap::new();
    };
    let clips = state
        .get("media")
        .and_then(|media| media.get("clips"))
        .and_then(serde_json::Value::as_array)
        .or_else(|| {
            state.as_array().and_then(|components| {
                components.iter().find_map(|component| {
                    component
                        .get("media")
                        .and_then(|media| media.get("clips"))
                        .and_then(serde_json::Value::as_array)
                })
            })
        });
    clips
        .into_iter()
        .flatten()
        .filter_map(|clip| {
            Some((
                json_string(clip, "src")?.to_owned(),
                json_string(clip, "caption")?.to_owned(),
            ))
        })
        .collect()
}
