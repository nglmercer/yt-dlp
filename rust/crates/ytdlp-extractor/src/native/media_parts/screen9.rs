/// Native Screen9 embed-configuration extractor.
pub struct Screen9Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Screen9Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Screen9Extractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Screen9 URL has no media ID")
            })?;
        let embed_url = format!("https://api.screen9.com/embed/{video_id}");
        let response = context.get(&embed_url)?;
        let html = String::from_utf8_lossy(response.body());
        let config = json_object_after_marker(&html, "var config").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Screen9 embed {video_id} has no player config"),
            )
        })?;
        let mut formats = Vec::new();
        if let Some(sources) = config.get("src").and_then(serde_json::Value::as_array) {
            for source in sources {
                let Some(media_url) = json_string(source, "src").filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let source_type = json_string(source, "type").unwrap_or_default();
                if source_type.eq_ignore_ascii_case("application/x-mpegURL")
                    || media_url
                        .split_once('?')
                        .map_or(media_url, |(path, _)| path)
                        .to_ascii_lowercase()
                        .ends_with(".m3u8")
                {
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": "hls",
                        "protocol": "m3u8_native",
                        "ext": "mp4",
                    }));
                } else if source_type.eq_ignore_ascii_case("video/mp4")
                    || media_url
                        .split_once('?')
                        .map_or(media_url, |(path, _)| path)
                        .to_ascii_lowercase()
                        .ends_with(".mp4")
                {
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": "http-mp4",
                        "format": "mp4",
                        "ext": "mp4",
                    }));
                }
            }
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Screen9 embed {video_id} has no playable sources"),
            )
        })?;
        let plugins = config.get("plugins");
        let title = plugins
            .and_then(|plugins| plugins.get("title"))
            .and_then(|title| json_string(title, "title"))
            .or_else(|| {
                plugins
                    .and_then(|plugins| plugins.get("googleAnalytics"))
                    .and_then(|analytics| json_string(analytics, "title"))
            })
            .or_else(|| {
                plugins
                    .and_then(|plugins| plugins.get("share"))
                    .and_then(|share| json_string(share, "mediaTitle"))
            })
            .map(str::to_owned);
        let description = plugins
            .and_then(|plugins| plugins.get("title"))
            .and_then(|title| json_string(title, "description"))
            .map(str::to_owned);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", json_string(&config, "poster"));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
