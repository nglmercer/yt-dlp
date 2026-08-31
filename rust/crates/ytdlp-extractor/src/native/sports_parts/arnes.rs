/// Native Arnes Video public-media API extractor.
pub struct ArnesExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ArnesExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ArnesExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Arnes URL has no ID")
            })?;
        let response = context.get_json(&format!(
            "https://video.arnes.si/api/public/video/{video_id}"
        ))?;
        let video = response.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Arnes video {video_id} has no data object"),
            )
        })?;
        let title = json_string(video, "title").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Arnes video {video_id} has no title"),
            )
        })?;
        let media = video
            .get("media")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Arnes video {video_id} has no media records"),
                )
            })?;
        let mut formats = Vec::new();
        for item in media {
            let Some(raw_url) = json_string(item, "url") else {
                continue;
            };
            let media_url = resolve_url("https://video.arnes.si", raw_url);
            let format_id = json_string(item, "format")
                .and_then(|value| value.strip_prefix("FORMAT_"))
                .map(str::to_owned);
            let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "format_note": json_string(item, "formatTranslation"),
                "width": json_i64(item, "width"),
                "height": json_i64(item, "height"),
                "ext": ext,
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Arnes video {video_id} has no playable media"),
            )
        })?;
        let channel = video.get("channel").unwrap_or(&serde_json::Value::Null);
        let channel_id = json_string(channel, "url");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "thumbnail",
            json_string(video, "thumbnailUrl")
                .map(|value| resolve_url("https://video.arnes.si", value)),
        );
        info.insert_if_some("description", json_string(video, "description"));
        info.insert_if_some("license", json_string(video, "license"));
        info.insert_if_some("creator", json_string(video, "author"));
        info.insert_if_some(
            "timestamp",
            json_string(video, "creationTime")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("channel", json_string(channel, "name"));
        info.insert_if_some("channel_id", channel_id);
        info.insert_if_some(
            "channel_url",
            channel_id.map(|value| format!("https://video.arnes.si/?channel={value}")),
        );
        info.insert_if_some(
            "duration",
            json_f64(video, "duration").map(|milliseconds| milliseconds / 1000.0),
        );
        info.insert_if_some("view_count", json_i64(video, "views"));
        info.insert_if_some("tags", video.get("hashtags").cloned());
        info.insert_if_some(
            "start_time",
            url_query_value(url, "t").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
