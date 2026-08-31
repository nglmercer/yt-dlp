/// Native Daystar Lightcast configuration/HLS extractor.
pub struct DaystarClipExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DaystarClipExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DaystarClipExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Daystar URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let iframe_url = Regex::new(r#"(?is)<iframe\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daystar clip {video_id} has no iframe"),
                )
            })?;
        let config_url = iframe_url.replace("player.php", "config2.php");
        let config_response = context.get(&config_url)?;
        let config_html = String::from_utf8_lossy(config_response.body());
        let sources = json_array_after_marker(&config_html, "sources")
            .and_then(|value| value.as_array().cloned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daystar clip {video_id} has no source list"),
                )
            })?;
        let mut formats = Vec::new();
        for source in sources {
            let Some(raw_url) = json_string(&source, "file") else {
                continue;
            };
            if json_string(&source, "type").map(|value| value.eq_ignore_ascii_case("m3u8"))
                != Some(true)
            {
                continue;
            }
            let media_url = resolve_url("https://www.lightcast.com/embed/", raw_url);
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Daystar clip {video_id} has no HLS source"),
            )
        })?;
        let thumbnail = Regex::new(r#"(?is)\bimage\s*:\s*["']([^"']+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&config_html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(&config_url, value.as_str()));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_meta_value(&html, "twitter:title")),
        );
        info.insert_if_some(
            "description",
            html_meta_value(&html, "og:description")
                .or_else(|| html_meta_value(&html, "twitter:description")),
        );
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
