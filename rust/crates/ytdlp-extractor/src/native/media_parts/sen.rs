/// Native Sen API/HLS video extractor.
pub struct SenExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SenExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SenExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Sen URL has no video ID")
            })?;
        let api_data = context.get_json(&format!(
            "https://api.sen.com/content/public/video/{video_id}"
        ))?;
        let nodes = api_data
            .get("data")
            .and_then(|data| data.get("nodes"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Sen video {video_id} has no API node list"),
                )
            })?;
        let player = nodes
            .iter()
            .find(|node| json_string(node, "id") == Some("player"));
        let manifest_url = player
            .and_then(|node| node.get("video"))
            .and_then(|video| json_string(video, "url"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!("https://vod.sen.com/videos/{video_id}/manifest.m3u8")
            });
        let details = nodes
            .iter()
            .find(|node| json_string(node, "id") == Some("details"))
            .and_then(|node| node.get("content"));
        let title = details
            .and_then(|content| content.get("title"))
            .and_then(|title| json_string(title, "text"))
            .map(str::to_owned);
        let description = details
            .and_then(|content| content.get("descriptions"))
            .and_then(serde_json::Value::as_array)
            .and_then(|values| values.first())
            .and_then(|description| json_string(description, "text"))
            .map(str::to_owned);
        let tags = details
            .and_then(|content| content.get("badges"))
            .and_then(serde_json::Value::as_array)
            .map(|badges| {
                badges
                    .iter()
                    .filter_map(|badge| json_string(badge, "text").map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .filter(|tags| !tags.is_empty());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert_if_some("tags", tags);
        info.insert("url", serde_json::json!(manifest_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": manifest_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
