/// Native Fathom share-page/API-state HLS extractor.
pub struct FathomExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FathomExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FathomExtractor {
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
        let share_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Fathom URL has no share ID")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let page_json = fathom_data_page(&html).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Fathom share {share_id} has no page state"),
            )
        })?;
        let props = page_json.get("props").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Fathom share {share_id} has no page props"),
            )
        })?;
        let call = props.get("call").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Fathom share {share_id} has no call data"),
            )
        })?;
        let video_id = json_value_string(call.get("id")).unwrap_or_else(|| share_id.clone());
        let media_url = json_string(call, "video_url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Fathom call {video_id} has no HLS URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            props
                .get("head")
                .and_then(|head| json_string(head, "title")),
        );
        info.insert_if_some("duration", json_f64(props, "duration"));
        info.insert_if_some(
            "timestamp",
            json_string(call, "started_at")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

fn fathom_data_page(html: &str) -> Option<serde_json::Value> {
    let patterns = [
        r#"(?is)<[a-z0-9]+\b[^>]*\bid\s*=\s*["']app["'][^>]*\bdata-page\s*=\s*["']([^"']*)"#,
        r#"(?is)<[a-z0-9]+\b[^>]*\bdata-page\s*=\s*["']([^"']*)["'][^>]*\bid\s*=\s*["']app["']"#,
    ];
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .and_then(|value| serde_json::from_str(&unescape_html_attribute(value.as_str())).ok())
    })
}
