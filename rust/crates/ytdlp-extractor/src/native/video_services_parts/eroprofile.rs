/// Native EroProfile HTML5-video extractor.
pub struct EroProfileExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EroProfileExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EroProfileExtractor {
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
                    "EroProfile URL has no display ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        if webpage.contains("You must be logged in to view this video.") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: EroProfile video {display_id} requires authenticated playback"),
            ));
        }
        let video_id = eroprofile_capture(
            &webpage,
            r"(?is)glbUpdViews\s*\(\s*'\d*'\s*,\s*'(\d+)'",
        )
        .or_else(|| eroprofile_capture(&webpage, r"(?is)p/report/video/(\d+)"));
        let title = eroprofile_capture(
            &webpage,
            r"(?is)Title:\s*</th>\s*<td[^>]*>(.*?)</td>",
        )
        .or_else(|| eroprofile_capture(&webpage, r"(?is)<h1[^>]*>(.*?)</h1>"))
        .map(|value| html_text_fragment(&value))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("EroProfile video {display_id} has no title"),
            )
        })?;
        let formats = html5_media_formats(url, &webpage);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("EroProfile video {display_id} has no HTML5 media sources"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(video_id.unwrap_or_else(|| display_id.clone())),
        );
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert("age_limit", serde_json::json!(18));
        info.insert_if_some("thumbnail", eroprofile_poster(url, &webpage));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("m4v")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn eroprofile_capture(html: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn eroprofile_poster(page_url: &str, html: &str) -> Option<String> {
    let poster = eroprofile_capture(
        html,
        r#"(?is)<video\b[^>]*\bposter\s*=\s*["']([^"']+)"#,
    )?;
    let poster = unescape_html_attribute(&poster);
    Some(resolve_url(page_url, &poster))
}
