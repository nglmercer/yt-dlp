/// Native Jove chapter-XML/direct-video extractor.
pub struct JoveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JoveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JoveExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Jove URL has no video ID")
            })?;
        let page_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let chapters_id = Regex::new(r#"(?i)/video-chapters\?videoid=(\d+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let chapters_url = format!("http://www.jove.com/video-chapters?videoid={chapters_id}");
        let chapters_response = context.get(&chapters_url)?;
        let chapters_xml = String::from_utf8_lossy(chapters_response.body());
        let media_url = jove_xml_attribute(&chapters_xml, "video").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Jove video {video_id} has no chapter video URL"),
            )
        })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", html_meta_value(&webpage, "citation_title"));
        info.insert_if_some(
            "thumbnail",
            html_meta_value(&webpage, "og:image")
                .map(|value| unescape_html_attribute(&value)),
        );
        info.insert_if_some(
            "description",
            jove_description(&webpage),
        );
        info.insert_if_some(
            "upload_date",
            html_meta_value(&webpage, "citation_publication_date")
                .and_then(|value| date_digits(&value)),
        );
        info.insert_if_some(
            "comment_count",
            html_meta_value(&webpage, "num_comments")
                .and_then(|value| value.split_whitespace().next()?.parse::<i64>().ok()),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "ext": extension,
                "protocol": "http",
            }]),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn jove_xml_attribute(xml: &str, attribute: &str) -> Option<String> {
    let matcher = Regex::new(&format!(
        r#"(?is)<[^>]+\b{}\s*=\s*["']([^"']+)["']"#,
        regex::escape(attribute)
    ))
    .ok()?;
    matcher
        .captures(xml)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
}

fn jove_description(webpage: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<div\b[^>]*\bid\s*=\s*["']section_body_summary["'][^>]*>\s*<p\b[^>]*\bclass\s*=\s*["']jove_content["'][^>]*>(.*?)</p\s*>"#,
    )
    .ok()?;
    matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
}
