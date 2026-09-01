/// Native Giant Bomb page-embedded stream extractor.
pub struct GiantBombExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GiantBombExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GiantBombExtractor {
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
                "Giant Bomb URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Giant Bomb URL has no ID")
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let video = html_data_json_attribute(&webpage, "video").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Giant Bomb page {display_id} has no video data"),
            )
        })?;
        let streams = video
            .get("videoStreams")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Giant Bomb video {video_id} has no stream map"),
                )
            })?;
        let mut formats = Vec::new();
        for (format_id, value) in streams {
            let Some(media_url) = value.as_str().filter(|value| {
                value.starts_with("http://") || value.starts_with("https://")
            }) else {
                continue;
            };
            let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
            if extension == "f4m" || format_id.starts_with("f4m") {
                continue;
            }
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "quality": giantbomb_quality(format_id),
                "ext": extension,
            });
            if extension == "m3u8" {
                format["protocol"] = serde_json::json!("m3u8_native");
                format["ext"] = serde_json::json!("mp4");
            }
            formats.push(format);
        }
        if formats.is_empty() {
            if let Some(youtube_id) = json_string(&video, "youtubeID") {
                return Ok(ExtractorResult::Redirect {
                    url: format!("https://www.youtube.com/watch?v={youtube_id}"),
                    ie_key: Some("Youtube".to_owned()),
                });
            }
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Giant Bomb video {video_id} has no native progressive/HLS stream; legacy HDS is unsupported"
                ),
            ));
        }
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Giant Bomb video {video_id} has no first stream URL"),
                )
            })?;
        let first_ext = formats
            .first()
            .and_then(|format| format.get("ext"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("mp4");
        let title = html_meta_value(&webpage, "og:title").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Giant Bomb page {display_id} has no title"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", html_meta_value(&webpage, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        info.insert_if_some("duration", json_i64(&video, "lengthSeconds"));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!(first_ext));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn giantbomb_quality(format_id: &str) -> i64 {
    match format_id {
        "f4m_low" => 0,
        "progressive_low" => 1,
        "f4m_high" => 2,
        "progressive_high" => 3,
        "f4m_hd" => 4,
        "progressive_hd" => 5,
        _ => -1,
    }
}
