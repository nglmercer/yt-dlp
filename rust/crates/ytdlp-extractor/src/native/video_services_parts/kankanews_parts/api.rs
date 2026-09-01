/// Native KankaNews page/API extractor.
pub struct KankaNewsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KankaNewsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KankaNewsExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "KankaNews URL has no display ID",
                )
            })?;
        let page_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let video_id = kankanews_video_id(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KankaNews page {display_id} has no OMS video ID"),
            )
        })?;
        let title = kankanews_title(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KankaNews page {display_id} has no title"),
            )
        })?;

        let mut api_request = Request::new("https://api-app.kankanews.com/kankan/pc/getvideo");
        api_request.update_query(&kankanews_query(&video_id));
        let api_response = context.request(&api_request)?;
        let response: serde_json::Value =
            serde_json::from_slice(api_response.body()).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid KankaNews API JSON for {video_id}: {error}"),
                )
            })?;
        let video = response
            .get("result")
            .and_then(|result| result.get("video"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("KankaNews API response for {video_id} has no video object"),
                )
            })?;
        let media_url = json_string(video, "videourl")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("KankaNews video {video_id} has no media URL"),
                )
            })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let protocol = if extension.eq_ignore_ascii_case("m3u8") {
            "m3u8_native"
        } else {
            "http"
        };
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("thumbnail", json_string(video, "titlepic"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": protocol,
                "protocol": protocol,
                "ext": extension,
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
