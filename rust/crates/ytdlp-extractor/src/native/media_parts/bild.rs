/// Native Bild.de JSON clip/source extractor.
pub struct BildExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BildExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BildExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Bild URL has no clip ID")
            })?;
        let api_url = url
            .split_once(".bild.html")
            .map(|(base, _)| format!("{base},view=json.bild.html"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Bild URL has no .bild.html suffix",
                )
            })?;
        let video_data = context.get_json(&api_url)?;
        let clip = video_data
            .get("clipList")
            .and_then(serde_json::Value::as_array)
            .and_then(|clips| clips.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Bild clip {video_id} has no clip data"),
                )
            })?;
        let mut formats = Vec::new();
        if let Some(sources) = clip.get("srces").and_then(serde_json::Value::as_array) {
            for source in sources {
                let Some(media_url) = json_string(source, "src").filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let source_type = json_string(source, "type").unwrap_or_default();
                if source_type.eq_ignore_ascii_case("application/x-mpegURL") {
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": "hls",
                        "protocol": "m3u8_native",
                        "ext": "mp4",
                    }));
                } else if source_type.eq_ignore_ascii_case("video/mp4") {
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": "http-mp4",
                        "ext": "mp4",
                    }));
                }
            }
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Bild clip {video_id} has no playable sources"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                json_string(&video_data, "title")
                    .map(unescape_html_attribute)
                    .unwrap_or_else(|| video_id.clone())
                    .trim()
                    .to_owned()
            ),
        );
        info.insert_if_some(
            "description",
            json_string(&video_data, "description").map(unescape_html_attribute),
        );
        info.insert_if_some("thumbnail", json_string(&video_data, "poster"));
        info.insert_if_some("duration", json_i64(&video_data, "durationSec"));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
