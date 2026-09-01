/// Native GameSpot page-embedded video extractor.
pub struct GameSpotExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GameSpotExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GameSpotExtractor {
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
        let page_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GameSpot URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let data_video = html_data_json_attribute(&webpage, "video").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GameSpot page {page_id} has no video data"),
            )
        })?;
        let title = json_string(&data_video, "title")
            .map(percent_decode)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GameSpot video {page_id} has no title"),
                )
            })?;
        let streams = data_video
            .get("videoStreams")
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GameSpot video {page_id} has no stream data"),
                )
            })?;
        let mut formats = Vec::new();
        if let Some(m3u8_url) = json_string(streams, "adaptive_stream") {
            formats.push(serde_json::json!({
                "url": m3u8_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
            formats.push(serde_json::json!({
                "url": m3u8_url.replace(".m3u8", ".mp4"),
                "format_id": "http",
                "protocol": "http",
                "ext": "mp4",
            }));
        }
        if let Some(mpd_url) = json_string(streams, "adaptive_dash") {
            formats.push(serde_json::json!({
                "url": mpd_url,
                "format_id": "dash",
                "protocol": "dash",
                "ext": "mp4",
            }));
        }
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(
                json_string(&data_video, "guid")
                    .unwrap_or(&format!("gs-{page_id}"))
            ),
        );
        info.insert("display_id", serde_json::json!(page_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", html_meta_value(&webpage, "description"));
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        if let Some(first_format) = formats.first() {
            info.insert_if_some("url", first_format.get("url"));
            info.insert_if_some("ext", first_format.get("ext"));
        }
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
