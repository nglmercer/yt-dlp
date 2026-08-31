/// Native El Trece TV Fusion-configuration/HLS extractor.
pub struct ElTreceTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ElTreceTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ElTreceTvExtractor {
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
        let slug = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "El Trece TV URL has no chapter slug",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let fusion = json_object_after_marker(&webpage, "Fusion.globalContent").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("El Trece TV chapter {slug} has no Fusion content"),
            )
        })?;
        let config = fusion
            .get("promo_items")
            .and_then(|value| value.get("basic"))
            .and_then(|value| value.get("embed"))
            .and_then(|value| value.get("config"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("El Trece TV chapter {slug} has no player config"),
                )
            })?;
        let hls_url = json_string(config, "m3u8")
            .map(|value| proto_relative_url(value, "https:"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("El Trece TV chapter {slug} has no HLS URL"),
                )
            })?;
        let video_id = Regex::new(r#"(?i)/([A-Za-z0-9_-]+)\.m3u8(?:/|$|[?#])"#)
            .ok()
            .and_then(|matcher| matcher.captures(&hls_url).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| slug.clone());
        let mut formats = vec![serde_json::json!({
            "url": hls_url,
            "format_id": "hls",
            "protocol": "m3u8_native",
            "ext": "mp4",
        })];
        if let Some(progressive_url) = eltrecetv_progressive_url(
            formats[0]
                .get("url")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default(),
        ) {
            formats.push(serde_json::json!({
                "url": progressive_url,
                "format_id": "http",
                "ext": "mp4",
            }));
        }
        let title = json_string(config, "title")
            .filter(|value| !value.is_empty())
            .map(unescape_html_attribute)
            .unwrap_or_else(|| slug.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", formats[0].get("url").cloned().unwrap_or(serde_json::Value::Null));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert_if_some(
            "thumbnail",
            json_string(config, "thumbnail").map(|value| proto_relative_url(value, "https:")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

fn eltrecetv_progressive_url(hls_url: &str) -> Option<String> {
    let suffix = "/tracks-v1a1/index.m3u8";
    hls_url
        .strip_suffix(suffix)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
