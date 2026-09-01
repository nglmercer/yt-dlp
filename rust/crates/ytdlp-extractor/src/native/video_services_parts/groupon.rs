/// Native Groupon deal-video playlist extractor.
pub struct GrouponExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GrouponExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GrouponExtractor {
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
        let deal_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Groupon URL has no deal ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let payload = json_object_after_marker(&webpage, "payload").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Groupon deal {deal_id} has no payload"),
            )
        })?;
        let mut entries = Vec::new();
        if let Some(videos) = payload
            .get("carousel")
            .and_then(|carousel| carousel.get("dealVideos"))
            .and_then(serde_json::Value::as_array)
        {
            for video in videos {
                let Some(provider) = json_string(video, "provider") else {
                    continue;
                };
                if !provider.eq_ignore_ascii_case("youtube") {
                    continue;
                }
                let Some(video_id) = ["media", "id", "baseURL"]
                    .iter()
                    .find_map(|key| json_value_string(video.get(*key)))
                else {
                    continue;
                };
                let target_url = if video_id.starts_with("http://")
                    || video_id.starts_with("https://")
                {
                    video_id
                } else {
                    format!("https://www.youtube.com/watch?v={video_id}")
                };
                let mut entry = native_url_result(&target_url);
                entry.insert("ie_key", serde_json::json!("Youtube"));
                entries.push(entry);
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(deal_id));
        info.insert_if_some("title", html_meta_value(&webpage, "og:title"));
        info.insert_if_some("description", html_meta_value(&webpage, "og:description"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
