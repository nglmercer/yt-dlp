/// Native Clubic/M6Web player-configuration extractor.
pub struct ClubicExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ClubicExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ClubicExtractor {
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
                "Clubic URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Clubic URL has no video ID")
            })?;
        let player_url = format!("http://player.m6web.fr/v1/player/clubic/{video_id}.html");
        let response = context.get(&player_url)?;
        let player_page = String::from_utf8_lossy(response.body());
        let config = json_object_after_marker(&player_page, "M6.Player.config").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Clubic video {video_id} has no M6 player configuration"),
            )
        })?;
        let video_info = config.get("videoInfo").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Clubic video {video_id} has no video metadata"),
            )
        })?;
        let sources = config
            .get("sources")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Clubic video {video_id} has no player sources"),
                )
            })?;

        let mut formats = Vec::new();
        for source in sources {
            let Some(media_url) = json_string(source, "src")
                .or_else(|| json_string(source, "url"))
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let quality_name = json_string(source, "streamQuality")
                .or_else(|| json_string(source, "label"))
                .unwrap_or("unknown");
            let quality = match quality_name.to_ascii_lowercase().as_str() {
                "sd" => 0,
                "hq" => 1,
                _ => -1,
            };
            formats.push(serde_json::json!({
                "format_id": quality_name,
                "url": media_url,
                "quality": quality,
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mp4"),
            }));
        }
        let first_format = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Clubic video {video_id} has no usable player sources"),
            )
        })?;
        let title = json_string(video_info, "title")
            .filter(|value| !value.is_empty())
            .unwrap_or(&video_id)
            .to_owned();
        let description = json_string(video_info, "description")
            .map(html_text_fragment)
            .filter(|value| !value.is_empty());
        let thumbnail = json_string(&config, "poster")
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let first_url = first_format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Clubic video {video_id} has an invalid first source"),
                )
            })?;
        let first_ext = first_format
            .get("ext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("mp4");

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!(first_ext));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
