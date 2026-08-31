/// Native This American Life archive/audio extractor.
pub struct ThisAmericanLifeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ThisAmericanLifeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ThisAmericanLifeExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "This American Life URL has no ID",
                )
            })?;
        let page_url = format!("http://www.thisamericanlife.org/radio-archives/episode/{video_id}");
        let webpage = context.get(&page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let stream_url =
            format!("http://stream.thisamericanlife.org/{video_id}/stream/{video_id}_64k.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("protocol", serde_json::json!("m3u8_native"));
        info.insert("ext", serde_json::json!("m4a"));
        info.insert("acodec", serde_json::json!("aac"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert("abr", serde_json::json!(64));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "twitter:title").unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "m4a",
                "acodec": "aac",
                "vcodec": "none",
                "abr": 64,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
