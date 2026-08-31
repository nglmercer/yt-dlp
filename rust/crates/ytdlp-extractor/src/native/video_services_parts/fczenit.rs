/// Native FC Zenit page-config/API progressive-video extractor.
pub struct FczenitExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FczenitExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FczenitExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "FC Zenit URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let config = json_object_after_marker(&webpage, "config").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FC Zenit video {video_id} has no player configuration"),
            )
        })?;
        let msi_id = json_string(&config, "video_id")
            .or_else(|| json_string(&config, "videoId"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FC Zenit video {video_id} has no MSI ID"),
                )
            })?;
        let api_url = format!("http://player.fc-zenit.ru/msi/video?video={msi_id}");
        let api_response = context.get(&api_url)?;
        let api_json: serde_json::Value = serde_json::from_slice(api_response.body()).map_err(
            |error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid FC Zenit API response for {video_id}: {error}"),
                )
            },
        )?;
        let data = api_json.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FC Zenit API response for {video_id} has no data"),
            )
        })?;
        let qualities = data
            .get("qualities")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FC Zenit video {video_id} has no quality list"),
                )
            })?;
        let mut formats = Vec::new();
        for quality in qualities {
            let Some(media_url) = json_string(quality, "url").filter(|value| !value.is_empty())
            else {
                continue;
            };
            let label = json_string(quality, "label")
                .filter(|value| !value.is_empty())
                .unwrap_or("http");
            formats.push(serde_json::json!({
                "format_id": label,
                "url": media_url,
                "height": label.parse::<i64>().ok(),
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mp4"),
            }));
        }
        let first_format = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FC Zenit video {video_id} has no usable media quality"),
            )
        })?;
        let first_url = first_format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FC Zenit video {video_id} has an invalid first quality"),
                )
            })?;
        let extension = first_format
            .get("ext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(data, "name"));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert_if_some("thumbnail", json_string(data, "preview"));
        info.insert_if_some("duration", json_f64(data, "duration"));
        info.insert_if_some("timestamp", json_i64(data, "date"));
        info.insert(
            "tags",
            serde_json::Value::Array(
                data.get("tags")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|tag| json_string(tag, "label"))
                    .map(|tag| serde_json::json!(tag))
                    .collect(),
            ),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
