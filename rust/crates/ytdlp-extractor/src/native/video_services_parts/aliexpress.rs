/// Native AliExpress Live embedded run-parameters/HLS extractor.
pub struct AliExpressLiveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AliExpressLiveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AliExpressLiveExtractor {
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
                    "AliExpress Live URL has no stream ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let data = json_object_after_marker(&webpage, "runParams").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("AliExpress Live stream {video_id} has no run parameters"),
            )
        })?;
        let stream_url = json_string(&data, "replyStreamUrl")
            .map(|value| proto_relative_url(value, "https:"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("AliExpress Live stream {video_id} has no HLS URL"),
                )
            })?;
        let extension = yt_dlp_core::determine_ext(Some(&stream_url), "unknown");
        if extension != "m3u8" {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: AliExpress Live native extractor only implements HLS streams, got {extension}"
                ),
            ));
        }
        let title = json_string(&data, "title")
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| video_id.clone());
        let uploader = data
            .get("followBar")
            .and_then(|value| json_string(value, "name"))
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let timestamp = json_f64(&data, "startTimeLong")
            .map(|milliseconds| (milliseconds / 1000.0).round() as i64);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert_if_some("thumbnail", json_string(&data, "coverUrl"));
        info.insert_if_some("uploader", uploader);
        info.insert_if_some("timestamp", timestamp);
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        info.insert("is_live", serde_json::json!(true));
        Ok(ExtractorResult::single(info))
    }
}
