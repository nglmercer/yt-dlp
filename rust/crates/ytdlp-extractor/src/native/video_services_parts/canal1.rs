/// Native Canal 1 transparent embedded-player wrapper.
pub struct Canal1Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Canal1Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Canal1Extractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Canal 1 URL has no display ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let embed_url = Regex::new(r#"(?is)"embedUrl"\s*:\s*"([^"]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| decode_canal1_url(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Canal 1 page {display_id} has no embedded player URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("_type", serde_json::json!("url_transparent"));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("url", serde_json::json!(embed_url));
        Ok(ExtractorResult::single(info))
    }
}

fn decode_canal1_url(value: &str) -> String {
    let quoted = format!("\"{value}\"");
    serde_json::from_str::<String>(&quoted)
        .unwrap_or_else(|_| value.replace(r"\/", "/").replace("&amp;", "&"))
}
