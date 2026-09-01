/// Native Karaoketv RTMP extractor.
pub struct KaraoketvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KaraoketvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KaraoketvExtractor {
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
                    "Karaoketv URL has no video ID",
                )
            })?;
        let webpage = String::from_utf8_lossy(context.get(url)?.body()).into_owned();
        let api_page_url = karaoketv_iframe_url(&webpage, "karaoke.co.il/api_play.php")
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Karaoketv video {video_id} has no API player URL"),
                )
            })?;
        let api_page = String::from_utf8_lossy(context.get(&api_page_url)?.body()).into_owned();
        let video_cdn_url = karaoketv_iframe_url(&api_page, "video-cdn.com/embed/iframe/")
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Karaoketv video {video_id} has no CDN player URL"),
                )
            })?;
        let video_cdn = String::from_utf8_lossy(context.get(&video_cdn_url)?.body()).into_owned();
        let play_path = karaoketv_play_path(&video_cdn).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Karaoketv video {video_id} has no RTMP play path"),
            )
        })?;
        let formats = karaoketv_formats(
            &play_path,
            karaoketv_servers(&video_cdn),
            &video_cdn_url,
        );
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Karaoketv video {video_id} has no RTMP servers"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", karaoketv_title(&webpage));
        info.insert(
            "url",
            first
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("flv"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
