/// Native GoodGame live-channel API and HLS extractor.
pub struct GoodGameExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GoodGameExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GoodGameExtractor {
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
        let channel_name = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GoodGame URL has no channel")
            })?;
        let response = context.get_json(&format!(
            "https://goodgame.ru/api/4/users/{channel_name}/stream"
        ))?;
        let stream_key = json_value_string(response.get("streamkey")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GoodGame channel {channel_name} has no stream key"),
            )
        })?;
        if !json_bool(&response, "status").unwrap_or(false) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("GoodGame channel {channel_name} is offline"),
            ));
        }
        let stream_url = format!("https://hls.goodgame.ru/manifest/{stream_key}_master.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(stream_key));
        info.insert(
            "url",
            serde_json::json!(stream_url.clone()),
        );
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
        info.insert("subtitles", serde_json::json!({}));
        info.insert("is_live", serde_json::json!(true));
        info.insert_if_some("title", json_string(&response, "title"));
        info.insert_if_some("channel", json_string(&response, "channelkey"));
        info.insert_if_some(
            "channel_id",
            json_value_string(response.get("id")),
        );
        info.insert_if_some("channel_url", json_string(&response, "link"));
        info.insert_if_some(
            "uploader",
            response
                .get("streamer")
                .and_then(|streamer| json_string(streamer, "username")),
        );
        info.insert_if_some(
            "uploader_id",
            response
                .get("streamer")
                .and_then(|streamer| json_value_string(streamer.get("id"))),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&response, "preview").map(|value| proto_relative_url(value, "https:")),
        );
        info.insert_if_some(
            "concurrent_view_count",
            json_i64(&response, "viewers"),
        );
        info.insert_if_some(
            "channel_follower_count",
            json_i64(&response, "followers"),
        );
        if json_bool(&response, "adult") == Some(true) {
            info.insert("age_limit", serde_json::json!(18));
        }
        Ok(ExtractorResult::single(info))
    }
}
