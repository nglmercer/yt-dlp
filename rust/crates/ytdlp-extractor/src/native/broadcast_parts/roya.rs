/// Native Roya TV live-channel API/HLS extractor.
pub struct RoyaLiveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RoyaLiveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RoyaLiveExtractor {
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
        let media_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Roya TV URL has no channel ID")
            })?;
        let stream_data = context.get_json(&format!(
            "https://ticket.roya-tv.com/api/v5/fastchannel/{media_id}"
        ))?;
        let stream_url = stream_data
            .get("data")
            .and_then(|data| json_string(data, "secured_url"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Roya channel {media_id} has no secured stream URL"),
                )
            })?;
        let title = context
            .get_json("https://backend.roya.tv/api/v01/channels/schedule-pagination")
            .ok()
            .and_then(|schedule| {
                schedule
                    .get("data")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|entries| entries.first())
                    .and_then(|entry| entry.get("channel"))
                    .filter(|channel| {
                        json_string(channel, "id") == Some(media_id.as_str())
                            || json_i64(channel, "id").map(|id| id.to_string())
                                == Some(media_id.clone())
                    })
                    .and_then(|channel| json_string(channel, "title"))
                    .map(str::to_owned)
            });

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(media_id));
        info.insert_if_some("title", title);
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert("url", serde_json::json!(stream_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
