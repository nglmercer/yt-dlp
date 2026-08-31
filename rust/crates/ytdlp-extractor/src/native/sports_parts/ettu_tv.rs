/// Native ETTU TV player-settings/stream-access HLS extractor.
pub struct EttuTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EttuTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EttuTvExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "ETTU TV URL has no ID")
            })?;
        let player_settings = context.get_json(&format!(
            "https://www.ettu.tv/api/v3/contents/{video_id}/player-settings?language=en&showTitle=true&device=desktop"
        ))?;
        let stream_access = json_string(&player_settings, "streamAccess")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ETTU TV content {video_id} has no stream access URL"),
                )
            })?;
        let mut request = Request::new(stream_access);
        request.set_method("POST").map_err(map_request_error)?;
        request.set_data(Some(Vec::new()));
        let stream_response = context.request(&request)?;
        let stream_response: serde_json::Value =
            serde_json::from_slice(stream_response.body()).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid ETTU TV stream response: {error}"),
                )
            })?;
        let stream_url = stream_response
            .get("data")
            .and_then(|data| json_string(data, "stream"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ETTU TV content {video_id} has no HLS stream"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&player_settings, "title"));
        info.insert_if_some(
            "description",
            player_settings
                .get("metaInformation")
                .and_then(|meta| json_string(meta, "competition")),
        );
        info.insert_if_some("thumbnail", json_string(&player_settings, "image"));
        info.insert_if_some(
            "timestamp",
            json_string(&player_settings, "date")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        if let Some(is_live) = player_settings.get("isLivestream").and_then(serde_json::Value::as_bool)
        {
            info.insert("is_live", serde_json::json!(is_live));
        }
        info.insert("url", serde_json::json!(stream_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
