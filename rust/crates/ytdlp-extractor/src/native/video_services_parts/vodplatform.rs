/// Native VOD Platform hidden-player-input extractor.
pub struct VodPlatformExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl VodPlatformExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for VodPlatformExtractor {
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
                    "VOD Platform URL has no embed ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let media_url = html_named_input_value(&webpage, "HiddenmyhHlsLink")
            .filter(|value| !value.is_empty())
            .or_else(|| {
                html_named_input_value(&webpage, "HiddenmyDashLink")
                    .filter(|value| !value.is_empty())
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("VOD Platform embed {video_id} has no HLS or DASH URL"),
                )
            })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4").to_ascii_lowercase();
        let lower_url = media_url.to_ascii_lowercase();
        if extension == "f4m"
            || extension == "smil"
            || lower_url.contains(".smil")
            || lower_url.contains(".f4m")
        {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: VOD Platform native extractor does not implement Wowza/SMIL manifests for {media_url}"
                ),
            ));
        }
        let (format_id, protocol) = match extension.as_str() {
            "m3u8" => ("hls", "m3u8_native"),
            "mpd" => ("dash", "http_dash_segments"),
            _ if lower_url.starts_with("rtmp://") || lower_url.starts_with("rtmps://") => {
                ("rtmp", "rtmp")
            }
            _ => ("http", "http"),
        };
        let title = html_meta_value(&webpage, "og:title")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| video_id.clone());
        let thumbnail = html_named_input_value(&webpage, "HiddenThumbnail")
            .filter(|value| !value.is_empty())
            .or_else(|| html_meta_value(&webpage, "og:image"))
            .filter(|value| !value.is_empty());
        let format = serde_json::json!({
            "id": format_id,
            "format_id": format_id,
            "url": media_url,
            "ext": "mp4",
            "protocol": protocol,
        });
        let selected_url = format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(selected_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("formats", serde_json::json!([format]));
        Ok(ExtractorResult::single(info))
    }
}
