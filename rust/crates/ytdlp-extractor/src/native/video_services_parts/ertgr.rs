/// Native ERT web-TV embedded-player extractor.
pub struct ErtWebtvEmbedExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ErtWebtvEmbedExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ErtWebtvEmbedExtractor {
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
        _context: &ExtractionContext,
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
                    "ERT web-TV URL has no video ID",
                )
            })?;
        let manifest_url = format!(
            "https://mediastream.ert.gr/vodedge/_definst_/mp4:dvrorigin/{video_id}/playlist.m3u8"
        );
        let thumbnail = url_query_value(url, "bgimg").and_then(|thumbnail| {
            (!thumbnail.is_empty()).then(|| {
                if thumbnail.starts_with("http") {
                    thumbnail
                } else {
                    format!("https://program.ert.gr{thumbnail}")
                }
            })
        });
        let format = serde_json::json!({
            "url": manifest_url,
            "format_id": "hls",
            "protocol": "m3u8_native",
            "ext": "mp4",
        });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(format!("VOD - {video_id}")));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert(
            "url",
            format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::json!([format]));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
