/// Native Glide shared-message page extractor.
///
/// Glide embeds a normal HTML5 source and poster on the share page. The
/// result is kept as a direct native format so the Rust downloader can handle
/// it without delegating to another runtime.
pub struct GlideExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GlideExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GlideExtractor {
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
                "Glide URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Glide URL has no ID")
            })?;

        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let video_url = glide_attribute(&html, r#"<source\b[^>]*\bsrc"#)
            .or_else(|| html_meta_value(&html, "og:video:secure_url"))
            .or_else(|| html_meta_value(&html, "og:video"))
            .or_else(|| html_meta_value(&html, "twitter:player:stream"))
            .map(|value| resolve_url(url, &proto_relative_url(&value, "https:")))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Glide page {video_id} has no video source"),
                )
            })?;
        let thumbnail = glide_attribute(
            &html,
            r#"<img\b[^>]*\bid\s*=\s*["']video-thumbnail["'][^>]*\bsrc"#,
        )
        .or_else(|| html_meta_value(&html, "og:image"))
        .map(|value| resolve_url(url, &proto_relative_url(&value, "https:")));
        let title = html_title_value(&html)
            .or_else(|| html_meta_value(&html, "og:title"))
            .unwrap_or_else(|| video_id.clone());
        let extension = yt_dlp_core::determine_ext(Some(&video_url), "mp4");

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(video_url.clone()));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert(
            "formats",
            serde_json::json!([{
                "format_id": "http",
                "url": video_url,
                "ext": extension,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

fn glide_attribute(html: &str, prefix: &str) -> Option<String> {
    let pattern = format!(r#"(?is){prefix}\s*=\s*["']([^"']+)["']"#);
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
        .filter(|value| !value.trim().is_empty())
}
