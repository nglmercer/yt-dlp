/// Native GodResource stream API extractor.
pub struct GodResourceExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GodResourceExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GodResourceExtractor {
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
                    "GodResource URL has no video ID",
                )
            })?;
        let data = context.get_json(&format!(
            "https://api.godresource.com/api/Streams/{video_id}"
        ))?;
        let stream_url = json_string(&data, "streamUrl")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GodResource video {video_id} has no stream URL"),
                )
            })?;
        let extension = yt_dlp_core::determine_ext(Some(stream_url), "mp4");
        let (format_id, protocol, format_ext) = match extension.as_str() {
            "m3u8" => ("hls", "m3u8_native", "mp4"),
            "mp4" => ("http", "http", "mp4"),
            _ => {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!("GodResource video {video_id} returned unsupported format {extension}"),
                ));
            }
        };
        let is_live = json_bool(&data, "isLive").unwrap_or(false);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(stream_url));
        info.insert("ext", serde_json::json!(format_ext));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": format_id,
                "protocol": protocol,
                "ext": format_ext,
                "is_live": is_live,
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        info.insert("title", serde_json::json!(json_string(&data, "title").unwrap_or("")));
        info.insert("is_live", serde_json::json!(is_live));
        info.insert_if_some("thumbnail", json_string(&data, "thumbnail"));
        info.insert_if_some("view_count", json_i64(&data, "views"));
        info.insert_if_some("channel", json_string(&data, "channelName"));
        info.insert_if_some("channel_id", godresource_value_string(data.get("channelId")));
        info.insert_if_some(
            "timestamp",
            json_string(&data, "streamDateCreated")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "modified_timestamp",
            json_string(&data, "streamDataModified")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        Ok(ExtractorResult::single(info))
    }
}

fn godresource_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .unwrap_or_else(|| value.to_string())
    })
}
