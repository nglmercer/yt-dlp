/// Native Ku6 page/API extractor. Ku6 publishes the page title in the HTML
/// document and the playable F4V URL in a small JSON endpoint; both are
/// consumed directly through the Rust request context.
pub struct Ku6Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Ku6Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Ku6Extractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Ku6 URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Ku6 URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(r#"(?is)<h1\b[^>]*\btitle\s*=\s*["'][^"']*["'][^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_owned())
            .or_else(|| html_title_value(&html))
            .unwrap_or_else(|| video_id.clone());
        let response = context.get_json(&format!(
            "http://v.ku6.com/fetchVideo4Player/{video_id}.html"
        ))?;
        let media_url = response
            .get("data")
            .and_then(|data| json_string(data, "f"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Ku6 video {video_id} has no playable URL"),
                )
            })?;
        let ext = yt_dlp_core::determine_ext(Some(media_url), "f4v");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": ext,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
