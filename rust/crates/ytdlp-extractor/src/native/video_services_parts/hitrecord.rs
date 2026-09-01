/// Native HitRecord API-backed MP4 extractor.
pub struct HitRecordExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HitRecordExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HitRecordExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "HitRecord URL has no ID")
            })?;
        let video = context.get_json(&format!(
            "https://hitrecord.org/api/web/records/{video_id}"
        ))?;
        let title = json_string(&video, "title").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HitRecord item {video_id} has no title"),
            )
        })?;
        let media_url = video
            .get("source_url")
            .and_then(|source| json_string(source, "mp4_url"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("HitRecord item {video_id} has no MP4 URL"),
                )
            })?;
        let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "protocol": "http",
                "ext": extension,
            }]),
        );
        info.insert_if_some(
            "description",
            json_string(&video, "body").map(html_text_fragment),
        );
        info.insert_if_some("duration", json_f64(&video, "duration").map(|value| value / 1000.0));
        info.insert_if_some("timestamp", json_i64(&video, "created_at_i"));
        if let Some(user) = video.get("user") {
            info.insert_if_some("uploader", json_string(user, "username"));
            info.insert_if_some("uploader_id", json_value_string(user.get("id")));
        }
        for (source, target) in [
            ("total_views_count", "view_count"),
            ("hearts_count", "like_count"),
            ("comments_count", "comment_count"),
        ] {
            info.insert_if_some(target, json_i64(&video, source));
        }
        if let Some(tags) = video.get("tags").and_then(serde_json::Value::as_array) {
            let tags = tags
                .iter()
                .filter_map(|tag| json_string(tag, "text"))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if !tags.is_empty() {
                info.insert("tags", serde_json::json!(tags));
            }
        }
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
