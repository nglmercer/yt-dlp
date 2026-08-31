/// Native Blogger video configuration extractor.
pub struct BloggerExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BloggerExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BloggerExtractor {
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
        let token_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Blogger URL has no token")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let data = json_object_after_marker(&html, "var VIDEO_CONFIG").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Blogger video {token_id} has no VIDEO_CONFIG object"),
            )
        })?;
        let streams = data
            .get("streams")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Blogger video {token_id} has no streams"),
                )
            })?;
        let mut formats = Vec::new();
        for stream in streams {
            let Some(play_url) = json_string(stream, "play_url") else {
                continue;
            };
            let ext = url_query_value(play_url, "mime")
                .and_then(|mime| mimetype_extension(Some(&mime)))
                .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(play_url), "mp4"));
            formats.push(serde_json::json!({
                "url": play_url,
                "format_id": json_value_string(stream.get("format_id")),
                "ext": ext,
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Blogger video {token_id} has no playable streams"),
            )
        })?;
        let video_id = json_string(&data, "iframe_id").unwrap_or(&token_id);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(video_id));
        info.insert_if_some("thumbnail", json_string(&data, "thumbnail"));
        info.insert_if_some(
            "duration",
            first
                .get("url")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| url_query_value(value, "dur"))
                .and_then(|value| yt_dlp_core::parse_duration(&value)),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
