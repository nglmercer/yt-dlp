/// Native münchen.tv live-player configuration extractor.
pub struct MuenchenTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MuenchenTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MuenchenTvExtractor {
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
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let playlist_data = json_array_after_marker(&webpage, "playlist:").ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "münchen.tv page has no playlist configuration",
                )
            })?;
        let playlist = playlist_data
            .as_array()
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "münchen.tv page has an empty playlist configuration",
                )
            })?;
        let video_id = json_string(playlist, "mediaid")
            .or_else(|| json_string(playlist, "id"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "münchen.tv playlist has no media ID",
                )
            })?;
        let sources = playlist
            .get("sources")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("münchen.tv stream {video_id} has no sources"),
                )
            })?;

        let mut formats = Vec::new();
        for (index, source) in sources.iter().enumerate() {
            let Some(media_url) = json_string(source, "file")
                .or_else(|| json_string(source, "src"))
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let extension = yt_dlp_core::determine_ext(Some(media_url), "unknown");
            let label = json_string(source, "label")
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("_{index}"));
            let format_id = if extension == "unknown" {
                label
            } else {
                format!("{extension}-{label}")
            };
            let protocol = if extension.eq_ignore_ascii_case("m3u8") {
                "m3u8_native"
            } else if extension.eq_ignore_ascii_case("smil") || media_url.contains(".smil") {
                "smil"
            } else {
                "http"
            };
            formats.push(serde_json::json!({
                "url": media_url,
                "tbr": json_i64(source, "label"),
                "ext": "mp4",
                "format_id": format_id,
                "protocol": protocol,
                "preference": if protocol == "smil" { -100 } else { 0 },
            }));
        }
        let first_format = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("münchen.tv stream {video_id} has no usable sources"),
            )
        })?;
        let title = html_meta_value(&webpage, "og:title")
            .map(|value| unescape_html_attribute(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| html_title_value(&webpage))
            .unwrap_or_else(|| "münchen.tv-Livestream".to_owned());
        let thumbnail = json_string(playlist, "image")
            .filter(|value| !value.is_empty())
            .map(|value| proto_relative_url(value, "https:"));
        let first_url = first_format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("münchen.tv stream {video_id} has an invalid first source"),
                )
            })?;

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!("live"));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("is_live", serde_json::json!(true));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
