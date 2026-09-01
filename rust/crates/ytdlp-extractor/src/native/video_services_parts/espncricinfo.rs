/// Native ESPN Cricinfo video-details API extractor.
pub struct EspnCricinfoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EspnCricinfoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EspnCricinfoExtractor {
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
                    "ESPN Cricinfo URL has no video ID",
                )
            })?;
        let mut endpoint = Request::new(
            "https://hs-consumer-api.espncricinfo.com/v1/pages/video/video-details",
        );
        endpoint.update_query(&[("videoId".to_owned(), video_id.clone())]);
        let response = context.request(&endpoint)?;
        let data = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid ESPN Cricinfo JSON for {video_id}: {error}"),
            )
        })?;
        let video = data.get("video").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ESPN Cricinfo video {video_id} has no video data"),
            )
        })?;
        let mut formats = Vec::new();
        if let Some(playbacks) = video.get("playbacks").and_then(serde_json::Value::as_array) {
            for playback in playbacks {
                let Some(media_url) = json_string(playback, "url")
                    .filter(|value| {
                        value.starts_with("http://") || value.starts_with("https://")
                    })
                else {
                    continue;
                };
                let playback_type = json_string(playback, "type").unwrap_or("");
                if playback_type == "HLS" {
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": "hls",
                        "protocol": "m3u8_native",
                        "ext": "mp4",
                    }));
                } else if playback_type == "AUDIO" {
                    let extension = yt_dlp_core::determine_ext(Some(media_url), "m4a");
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": "audio",
                        "protocol": "http",
                        "ext": extension,
                        "vcodec": "none",
                    }));
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ESPN Cricinfo video {video_id} has no playable playback URLs"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let published = json_string(video, "publishedAt")
            .or_else(|| json_string(video, "recordedAt"))
            .map(str::to_owned);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(video, "title"));
        info.insert_if_some("description", json_string(video, "summary"));
        info.insert_if_some("upload_date", published.as_deref().and_then(date_digits));
        info.insert_if_some("timestamp", published.and_then(parse_timestamp));
        info.insert_if_some("duration", json_f64(video, "duration"));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
