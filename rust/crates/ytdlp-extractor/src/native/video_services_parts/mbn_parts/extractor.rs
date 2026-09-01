pub struct MbnExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MbnExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MbnExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "MBN URL has no video ID")
            })?;
        let webpage = mbn_page(context, url)?;
        let content_class_code = mbn_content_class_code(&webpage);
        let media_info = mbn_media_info(context, &video_id, &content_class_code)?;
        let formats = mbn_formats(context, &media_info, &video_id)?;
        let first_format = formats.first().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MBN video {video_id} has no first format"),
            )
        })?;
        let first_url = first_format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("MBN video {video_id} has an invalid first format"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("title", json_string(&media_info, "movie_title"));
        info.insert_if_some("duration", json_i64(&media_info, "play_sec"));
        info.insert_if_some(
            "release_date",
            json_string(&media_info, "bcast_date").and_then(date_digits),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&media_info, "movie_start_Img").and_then(mbn_http_url),
        );
        info.insert_if_some("series", json_string(&media_info, "prog_nm"));
        info.insert_if_some(
            "episode_number",
            json_i64(&media_info, "ad_contentnumber"),
        );
        Ok(ExtractorResult::single(info))
    }
}
