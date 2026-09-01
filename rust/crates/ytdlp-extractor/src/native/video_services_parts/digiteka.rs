/// Native Digiteka/Ultimedia player-configuration extractor.
pub struct DigitekaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DigitekaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DigitekaExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Digiteka URL has no video ID",
                )
            })?;
        let response = context.get_json(&format!(
            "https://www.ultimedia.com/player/getConf/01836272/1/{video_id}"
        ))?;
        let video_info = response.get("video").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Digiteka video {video_id} has no player data"),
            )
        })?;
        let mut formats = Vec::new();
        if let Some(hls_url) = video_info
            .get("media_sources")
            .and_then(|sources| sources.get("hls"))
            .and_then(|hls| hls.get("hls_auto"))
            .and_then(serde_json::Value::as_str)
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            formats.push(serde_json::json!({
                "url": hls_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        if let Some(mp4_sources) = video_info
            .get("media_sources")
            .and_then(|sources| sources.get("mp4"))
            .and_then(serde_json::Value::as_object)
        {
            for (format_id, value) in mp4_sources {
                let Some(media_url) = value
                    .as_str()
                    .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
                else {
                    continue;
                };
                let height = format_id
                    .split_once('_')
                    .and_then(|(_, height)| height.parse::<i64>().ok());
                let mut format = serde_json::Map::new();
                format.insert("url".to_owned(), serde_json::json!(media_url));
                format.insert("format_id".to_owned(), serde_json::json!(format_id));
                format.insert("ext".to_owned(), serde_json::json!("mp4"));
                format.insert("protocol".to_owned(), serde_json::json!("http"));
                if let Some(height) = height {
                    format.insert("height".to_owned(), serde_json::json!(height));
                }
                formats.push(serde_json::Value::Object(format));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Digiteka video {video_id} has no playable media sources"),
            ));
        }
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(video_id));
        output.insert_if_some("title", json_string(video_info, "title"));
        output.insert_if_some(
            "thumbnail",
            json_string(video_info, "image").filter(|value| {
                value.starts_with("http://") || value.starts_with("https://")
            }),
        );
        output.insert_if_some("duration", json_i64(video_info, "duration"));
        output.insert_if_some("timestamp", json_i64(video_info, "creationDate"));
        output.insert_if_some("uploader_id", json_string(video_info, "ownerId"));
        output.insert("url", first_url);
        output.insert("ext", serde_json::json!("mp4"));
        output.insert("formats", serde_json::Value::Array(formats));
        output.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(output))
    }
}
