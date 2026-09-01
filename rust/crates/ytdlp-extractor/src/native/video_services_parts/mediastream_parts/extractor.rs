/// Native MediaStream player configuration extractor.
pub struct MediaStreamExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MediaStreamExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MediaStreamExtractor {
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
                    "MediaStream URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        for message in [
            "Debido a tu ubicación no puedes ver el contenido",
            "You are not allowed to watch this video: Geo Fencing Restriction",
            "Este contenido no está disponible en tu zona geográfica.",
            "El contenido sólo está disponible dentro de",
        ] {
            if html.contains(message) {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: MediaStream video {video_id} is geo-restricted and native geo handling is not implemented"
                    ),
                ));
            }
        }
        let config = json_object_after_marker(&html, "window.MDSTRM.OPTIONS").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MediaStream video {video_id} has no player configuration"),
            )
        })?;
        let formats = mediastream_source_formats(url, &html, &config);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MediaStream video {video_id} has no playable sources"),
            ));
        }
        let title = html_meta_value(&html, "og:title")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| json_string(&config, "title").map(str::to_owned))
            .unwrap_or_else(|| video_id.clone());
        let is_live = json_string(&config, "type") == Some("live");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            html_meta_value(&html, "og:description").map(|value| html_text_fragment(&value)),
        );
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("formats", serde_json::Value::Array(formats.clone()));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("is_live", serde_json::json!(is_live));
        if is_live {
            info.insert("live_status", serde_json::json!("is_live"));
        }
        if let Some(first) = formats.first() {
            info.insert_if_some("url", first.get("url").cloned());
            info.insert_if_some("ext", first.get("ext").cloned());
        }
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
