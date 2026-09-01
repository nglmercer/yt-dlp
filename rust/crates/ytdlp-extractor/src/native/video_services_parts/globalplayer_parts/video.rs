/// Native Global Player video-page extractor.
pub struct GlobalPlayerVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GlobalPlayerVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GlobalPlayerVideoExtractor {
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
                    "Global Player video URL has no ID",
                )
            })?;
        let props = globalplayer_page_props(url, &video_id, context)?;
        let meta = props.get("videoData").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player video {video_id} has no video data"),
            )
        })?;
        let media_url = globalplayer_url(meta.get("url")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Global Player video {video_id} has no media URL"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::Value::Array(vec![globalplayer_format(&media_url, "mp4", false)]),
        );
        globalplayer_insert_meta(&mut info, meta);
        Ok(ExtractorResult::single(info))
    }
}
