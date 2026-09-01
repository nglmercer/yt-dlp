/// Native Expressen/Di article data and stream extractor.
pub struct ExpressenExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ExpressenExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ExpressenExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Expressen URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let tracking_info = html_data_json_attribute(&webpage, "video-tracking-info").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Expressen page {display_id} has no video tracking data"),
            )
        })?;
        let article_data = html_data_json_attribute(&webpage, "article-data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Expressen page {display_id} has no article data"),
            )
        })?;
        let video_id = json_value_string(tracking_info.get("contentId")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Expressen page {display_id} has no content ID"),
            )
        })?;
        let stream_url = json_string(&article_data, "stream")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Expressen video {video_id} has no stream URL"),
                )
            })?;
        let extension = yt_dlp_core::determine_ext(Some(&stream_url), "mp4");
        let is_hls = extension.eq_ignore_ascii_case("m3u8");
        let format = serde_json::json!({
            "url": stream_url,
            "format_id": if is_hls { "hls" } else { "http" },
            "protocol": if is_hls { "m3u8_native" } else { "http" },
            "ext": if is_hls { "mp4" } else { extension.as_str() },
        });
        let title = json_string(&tracking_info, "titleRaw")
            .filter(|value| !value.is_empty())
            .or_else(|| json_string(&article_data, "title"))
            .map(str::to_owned);
        let description = json_string(&tracking_info, "descriptionRaw").map(str::to_owned);
        let thumbnail = json_string(&tracking_info, "socialMediaImage")
            .or_else(|| json_string(&article_data, "image"))
            .map(str::to_owned);
        let duration = json_i64(&tracking_info, "videoTotalSecondsDuration")
            .or_else(|| json_i64(&article_data, "totalSecondsDuration"));
        let timestamp = json_string(&tracking_info, "publishDate")
            .map(str::to_owned)
            .and_then(parse_timestamp);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some("duration", duration);
        info.insert_if_some("timestamp", timestamp);
        info.insert("url", serde_json::json!(stream_url));
        info.insert(
            "ext",
            format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::json!([format]));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
