/// Native Elonet embedded-source extractor.
pub struct ElonetExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ElonetExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ElonetExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Elonet URL has no record ID")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let raw_sources = Regex::new(
            r#"(?is)\bid\s*=\s*['"]video-data['"][^>]*\bdata-video-sources\s*=\s*['"]([^'"]+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Elonet record {video_id} has no video source data"),
            )
        })?;
        let sources: serde_json::Value = serde_json::from_str(&raw_sources).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Elonet source data for {video_id}: {error}"),
            )
        })?;
        let media_url = sources
            .as_array()
            .and_then(|sources| sources.first())
            .and_then(|source| json_string(source, "src"))
            .filter(|value| !value.is_empty())
            .map(|value| resolve_url(url, value))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Elonet record {video_id} has no primary media URL"),
                )
            })?;
        let stream_ext =
            yt_dlp_core::determine_ext(Some(&media_url), "unknown").to_ascii_lowercase();
        let (format_id, protocol) = match stream_ext.as_str() {
            "m3u8" => ("hls", "m3u8_native"),
            "mpd" => ("dash", "http_dash_segments"),
            other => {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: Elonet native extractor does not implement {other} stream format"
                    ),
                ));
            }
        };
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", html_meta_value(&html, "og:title"));
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": format_id,
                "protocol": protocol,
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
