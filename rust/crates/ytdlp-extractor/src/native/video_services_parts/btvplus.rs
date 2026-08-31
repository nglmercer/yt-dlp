/// Native bTV Plus page/player-configuration extractor.
pub struct BtvPlusExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BtvPlusExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BtvPlusExtractor {
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
                    "bTV Plus URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let player_url = Regex::new(r##"(?i)\bvar\s+videoUrl\s*=\s*["']([^"']+)"##)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url("https://btvplus.bg", value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("bTV Plus video {video_id} has no player URL"),
                )
            })?;
        let player_config = context.get_json(&player_url)?;
        let config = player_config
            .get("config")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("bTV Plus player {video_id} has no config script"),
                )
            })?;
        let videojs_data = json_object_after_marker(config, "videojs(").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("bTV Plus player {video_id} has no videojs data"),
            )
        })?;
        let sources = videojs_data
            .get("sources")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("bTV Plus player {video_id} has no source list"),
                )
            })?;
        let mut formats = Vec::new();
        for source in sources {
            let Some(source_url) = json_string(source, "src").filter(|value| !value.is_empty())
            else {
                continue;
            };
            let source_type = json_string(source, "type").unwrap_or_default();
            let extension = btvplus_source_extension(source_type, source_url);
            match extension.as_str() {
                "m3u8" => formats.push(serde_json::json!({
                    "url": source_url,
                    "format_id": "hls",
                    "protocol": "m3u8_native",
                    "ext": "mp4",
                })),
                "mp4" | "webm" | "ogv" => formats.push(serde_json::json!({
                    "url": source_url,
                    "format_id": "http",
                    "protocol": "http",
                    "ext": extension,
                })),
                _ => {
                    return Err(ExtractorError::new(
                        ExtractorErrorKind::Unsupported,
                        format!(
                            "TODO: bTV Plus native extractor does not implement source type {source_type}"
                        ),
                    ));
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("bTV Plus video {video_id} has no playable sources"),
            ));
        }
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let title = html_meta_value(&webpage, "og:title")
            .or_else(|| {
                html_element_by_class(&webpage, "product-title")
                    .map(|value| html_text_fragment(&value))
            })
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        info.insert_if_some("description", html_meta_value(&webpage, "og:description"));
        info.insert("url", first_url);
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn btvplus_source_extension(source_type: &str, source_url: &str) -> String {
    let source_type = source_type.to_ascii_lowercase();
    if source_type.contains("mpegurl") || source_type.contains("m3u8") {
        return "m3u8".to_owned();
    }
    let extension = yt_dlp_core::determine_ext(Some(source_url), "unknown").to_ascii_lowercase();
    if extension != "unknown" {
        return extension;
    }
    match source_type.as_str() {
        "video/mp4" => "mp4".to_owned(),
        "video/webm" => "webm".to_owned(),
        "video/ogg" => "ogv".to_owned(),
        _ => "unknown".to_owned(),
    }
}
