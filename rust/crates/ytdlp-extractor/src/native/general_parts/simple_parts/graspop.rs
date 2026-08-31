/// Native Graspop festival extractor. The festival API returns the HLS asset
/// and poster metadata in one JSON response; the native downloader consumes
/// the returned manifest URL.
pub struct GraspopExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GraspopExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GraspopExtractor {
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
                "Graspop URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Graspop URL has no ID")
            })?;
        let metadata = context.get_json(&format!(
            "https://tv.proximus.be/MWC/videocenter/festivals/{video_id}/stream"
        ))?;
        let asset_url = metadata
            .get("source")
            .and_then(|source| json_string(source, "assetUri"))
            .filter(|value| !value.is_empty())
            .map(|value| url_with_scheme(value, "http"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Graspop video {video_id} has no HLS asset"),
                )
            })?;
        let extension = "mp4";
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                json_string(&metadata, "name")
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&video_id)
            ),
        );
        info.insert("url", serde_json::json!(asset_url.clone()));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": asset_url,
                "format_id": "hls",
                "ext": extension,
                "protocol": "m3u8_native",
            }]),
        );
        info.insert_if_some(
            "thumbnail",
            metadata
                .get("source")
                .and_then(|source| json_string(source, "poster")),
        );
        Ok(ExtractorResult::single(info))
    }
}
