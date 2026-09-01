/// Native Frontend Masters lesson source/transcript extractor.
pub struct FrontendMastersExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FrontendMastersExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FrontendMastersExtractor {
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
        let lesson_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Frontend Masters URL has no lesson ID",
                )
            })?;
        let mut formats = Vec::new();
        for (extension, qualities) in [
            ("webm", [("low", 360, 480), ("mid", 720, 1280), ("high", 1080, 1920)]),
            ("mp4", [("low", 360, 480), ("mid", 720, 1280), ("high", 1080, 1920)]),
        ] {
            for (quality, height, width) in qualities {
                let endpoint = format!(
                    "{FRONTEND_MASTERS_API}/video/{lesson_id}/source?f={extension}&r={height}"
                );
                let response = context
                    .get_json(&endpoint)
                    .or_else(|error| {
                        if error.kind == ExtractorErrorKind::Network {
                            Ok(serde_json::Value::Null)
                        } else {
                            Err(error)
                        }
                    })?;
                let Some(format_url) = response
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| {
                        value.starts_with("http://") || value.starts_with("https://")
                    })
                else {
                    continue;
                };
                formats.push(serde_json::json!({
                    "url": format_url,
                    "ext": extension,
                    "format_id": format!("{extension}-{quality}"),
                    "width": width,
                    "height": height,
                }));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Frontend Masters lesson {lesson_id} requires account-authenticated \
                     source access, which is not configured in native Rust"
                ),
            ));
        }
        let mut subtitles = serde_json::Map::new();
        subtitles.insert(
            "en".to_owned(),
            serde_json::json!([{
                "url": format!("{FRONTEND_MASTERS_API}/transcripts/{lesson_id}.vtt")
            }]),
        );
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(lesson_id));
        info.insert("title", serde_json::json!(lesson_id));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::Value::Object(subtitles));
        Ok(ExtractorResult::single(info))
    }
}
