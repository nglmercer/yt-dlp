/// Native Mocha API/video extractor.
pub struct MochaVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MochaVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MochaVideoExtractor {
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
                "Mocha URL did not match its native pattern",
            )
        })?;
        let video_slug = captures
            .name("video_slug")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mocha URL has no video slug")
            })?;
        let video = mocha_video_detail(context, url)?;
        let video_id = json_value_string(video.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Mocha video {video_slug} has no numeric ID"),
            )
        })?;
        let formats = mocha_formats(&video, &video_id)?;
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "display_id",
            serde_json::json!(json_string(&video, "slug").unwrap_or(&video_slug)),
        );
        info.insert_if_some("title", json_string(&video, "name"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("subtitles", Some(serde_json::json!({})));
        info.insert_if_some("description", json_string(&video, "description"));
        info.insert_if_some(
            "duration",
            json_i64(&video, "durationS")
                .or_else(|| mocha_number(&video, "durationS").map(|value| value as i64)),
        );
        info.insert_if_some("view_count", json_i64(&video, "total_view"));
        info.insert_if_some("like_count", json_i64(&video, "total_like"));
        info.insert_if_some("dislike_count", json_i64(&video, "total_unlike"));
        info.insert_if_some("comment_count", json_i64(&video, "total_comment"));
        info.insert_if_some("thumbnail", json_string(&video, "image_path_thumb"));
        info.insert_if_some(
            "timestamp",
            mocha_number(&video, "publish_time").map(|value| (value / 1000.0) as i64),
        );
        info.insert_if_some("is_live", json_bool(&video, "isLive"));
        if let Some(channel) = video
            .get("channels")
            .and_then(serde_json::Value::as_array)
            .and_then(|channels| channels.first())
        {
            info.insert_if_some("channel", json_string(channel, "name"));
            info.insert_if_some("channel_id", channel.get("id").cloned());
            info.insert_if_some("channel_follower_count", json_i64(channel, "numfollow"));
        }
        let categories = video
            .get("categories")
            .and_then(serde_json::Value::as_array)
            .map(|categories| {
                categories
                    .iter()
                    .filter_map(|category| json_string(category, "categoryname"))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|categories| !categories.is_empty());
        info.insert_if_some("categories", categories);
        info.insert("url", first_url);
        let first_ext = info
            .get("formats")
                .and_then(serde_json::Value::as_array)
                .and_then(|formats| formats.first())
                .and_then(|format| format.get("ext"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        info.insert_if_some("ext", first_ext);
        Ok(ExtractorResult::single(info))
    }
}

fn mocha_number(video: &serde_json::Value, key: &str) -> Option<f64> {
    video.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|value| value as f64))
            .or_else(|| value.as_u64().map(|value| value as f64))
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}
