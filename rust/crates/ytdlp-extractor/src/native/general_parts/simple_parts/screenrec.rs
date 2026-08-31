/// Native ScreenRec page extractor. The player configuration embeds an HLS
/// URL and the page supplies OpenGraph metadata.
pub struct ScreenRecExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ScreenRecExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ScreenRecExtractor {
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
                "ScreenRec URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "ScreenRec URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let matcher = Regex::new(r#"(?is)\bcustomUrl\s*:\s*(["'])(?P<url>(?:(?!\1).)+)\1"#)
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid ScreenRec player matcher: {error}"),
                )
            })?;
        let captures = matcher.captures(&html).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ScreenRec video {video_id} has no HLS URL"),
            )
        })?;
        let media_url = captures
            .name("url")
            .map(|value| unescape_html_attribute(value.as_str()))
            .filter(|value| !value.is_empty())
            .map(|value| proto_relative_url(&value, "https:"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ScreenRec video {video_id} has an empty HLS URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title")
                    .or_else(|| html_title_value(&html))
                    .unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
