/// Native Kick clip extractor.
pub struct KickClipExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KickClipExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KickClipExtractor {
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
        let clip_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Kick clip URL has no clip ID")
            })?;
        let response = kick_api_json(context, &format!("v2/clips/{clip_id}/play"))?;
        let clip = response.get("clip").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kick clip {clip_id} has no clip data"),
            )
        })?;
        let media_url = json_string(clip, "clip_url")
            .and_then(kick_valid_url)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kick clip {clip_id} has no media URL"),
                )
            })?;
        let format_id = if yt_dlp_core::determine_ext(Some(&media_url), "mp4") == "m3u8" {
            "hls"
        } else {
            "source"
        };
        let formats = vec![kick_media_format(&media_url, format_id)];
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let channel = clip.get("channel").unwrap_or(&serde_json::Value::Null);
        let creator = clip.get("creator").unwrap_or(&serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(clip_id));
        info.insert_if_some("title", json_string(clip, "title"));
        info.insert_if_some("channel", json_string(channel, "slug"));
        info.insert_if_some("channel_id", json_value_string(channel.get("id")));
        info.insert_if_some("uploader", json_string(creator, "username"));
        info.insert_if_some("uploader_id", json_value_string(creator.get("id")));
        info.insert_if_some("thumbnail", kick_optional_url(clip.get("thumbnail_url")));
        info.insert_if_some("duration", json_f64(clip, "duration"));
        info.insert_if_some(
            "categories",
            kick_category_names(clip.get("category")),
        );
        info.insert_if_some(
            "timestamp",
            kick_timestamp(json_string(clip, "created_at")),
        );
        info.insert_if_some("upload_date", json_string(clip, "created_at").and_then(date_digits));
        info.insert_if_some("view_count", json_i64(clip, "views"));
        info.insert_if_some("like_count", json_i64(clip, "likes"));
        info.insert_if_some("age_limit", kick_age_limit(clip.get("is_mature")));
        info.insert(
            "url",
            first
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}
