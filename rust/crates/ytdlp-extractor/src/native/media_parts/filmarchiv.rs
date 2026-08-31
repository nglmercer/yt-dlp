/// Native FilmArchiv.at deterministic CDN/HLS extractor.
pub struct FilmArchivExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FilmArchivExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FilmArchivExtractor {
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
        let media_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "FilmArchiv URL has no media ID")
            })?;
        let page = context.get(url)?;
        let html = String::from_utf8_lossy(page.body());
        let path = format!("{}/{}", &media_id[..6], &media_id[6..]);
        let stream_url = format!(
            "https://cdn.filmarchiv.at/{path}_v1_sv1/playlist.m3u8"
        );
        let title = Regex::new(r"(?is)<title-div\b[^>]*>(.*?)</title-div\s*>")
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .or_else(|| html_meta_value(&html, "og:title"));
        let description = html_element_by_class(&html, "border-base-content")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| html_meta_value(&html, "description"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(media_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert(
            "thumbnail",
            serde_json::json!(format!("https://cdn.filmarchiv.at/{path}_v1/poster.jpg")),
        );
        info.insert("url", serde_json::json!(stream_url));
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
        Ok(ExtractorResult::single(info))
    }
}
