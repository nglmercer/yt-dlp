/// Native GB News Simplestream extractor.
pub struct GbNewsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GbNewsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GbNewsExtractor {
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
            .map(|value| value.as_str().rsplit('/').next().unwrap_or(value.as_str()).to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GB News URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let attributes = gbnews_video_attributes(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GB News page {display_id} has no Simplestream video element"),
            )
        })?;
        let data_id = attributes
            .get("data-id")
            .cloned()
            .unwrap_or_else(|| "GB003".to_owned());
        let data_env = attributes
            .get("data-env")
            .cloned()
            .unwrap_or_else(|| "production".to_owned());
        let meta_url = format!(
            "https://mm-v2.simplestream.com/ssmp/api.php?id={data_id}&env={data_env}"
        );
        let metadata = context.get_json(&meta_url)?;
        let endpoint = metadata
            .get("response")
            .and_then(|response| json_string(response, "api_hostname"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GB News page {display_id} has no Simplestream API host"),
                )
            })?
            .trim_end_matches('/')
            .to_owned();
        let uvid = attributes.get("data-uvid").cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GB News page {display_id} has no Simplestream video ID"),
            )
        })?;
        let video_type = match attributes.get("data-type").map(String::as_str) {
            None | Some("") | Some("vod") => "show",
            Some(value) => value,
        };
        let mut stream_request =
            Request::new(format!("{endpoint}/api/{video_type}/stream/{uvid}"));
        stream_request.update_query(&[
            (
                "key".to_owned(),
                attributes.get("data-key").cloned().unwrap_or_default(),
            ),
            ("platform".to_owned(), "safari".to_owned()),
        ]);
        let stream_response = context.request(&stream_request)?;
        let stream_data = serde_json::from_slice::<serde_json::Value>(stream_response.body())
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid GB News stream JSON for {uvid}: {error}"),
                )
            })?;
        if gbnews_truthy(stream_data.get("drm")) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: GB News video {uvid} requires DRM playback"),
            ));
        }
        let stream_url = stream_data
            .get("response")
            .and_then(|response| json_string(response, "stream"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GB News video {uvid} has no HLS stream"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(uvid));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", html_meta_value(&webpage, "og:title"));
        info.insert_if_some("description", html_meta_value(&webpage, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        info.insert(
            "url",
            serde_json::json!(stream_url),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        info.insert("is_live", serde_json::json!(video_type == "live"));
        Ok(ExtractorResult::single(info))
    }
}

fn gbnews_video_attributes(html: &str) -> Option<std::collections::HashMap<String, String>> {
    let matcher = Regex::new(
        r#"(?is)<[a-z0-9]+\b[^>]*\bclass\s*=\s*["'][^"']*\bsimplestream\b[^"']*["'][^>]*>"#,
    )
    .ok()?;
    matcher.captures_iter(html).flatten().find_map(|captures| {
        let tag = captures.get(0)?.as_str();
        let class = gbnews_attribute(tag, "class")?;
        if class.to_ascii_lowercase().contains("sidebar") {
            return None;
        }
        let mut attributes = std::collections::HashMap::new();
        for name in ["data-id", "data-env", "data-uvid", "data-type", "data-key"] {
            if let Some(value) = gbnews_attribute(tag, name) {
                attributes.insert(name.to_owned(), value);
            }
        }
        Some(attributes)
    })
}

fn gbnews_attribute(tag: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"(?is)\b{}\s*=\s*["']([^"']*)"#, regex::escape(name));
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(tag).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| unescape_html_attribute(value.as_str())))
}

fn gbnews_truthy(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::Number(value)) => value.as_i64().is_some_and(|value| value != 0),
        Some(serde_json::Value::String(value)) => !value.is_empty() && value != "0",
        _ => false,
    }
}
