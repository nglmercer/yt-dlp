/// Native Canal Alpha server-state extractor.
pub struct CanalAlphaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CanalAlphaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CanalAlphaExtractor {
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
                    "Canal Alpha URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let state = json_object_after_marker(&webpage, "window.__SERVER_STATE__").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Canal Alpha video {video_id} has no server state"),
            )
        })?;
        let data = state
            .get("1")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("data"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Canal Alpha video {video_id} has invalid server state"),
                )
            })?;
        let video = data.get("video").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Canal Alpha video {video_id} has no video state"),
            )
        })?;
        let mut formats = Vec::new();
        if let Some(progressive) = video.get("mp4").and_then(serde_json::Value::as_array) {
            for source in progressive {
                let Some(media_url) = json_string(source, "$url").filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let mut format = serde_json::Map::new();
                format.insert("url".to_owned(), serde_json::json!(media_url));
                format.insert("ext".to_owned(), serde_json::json!("mp4"));
                format.insert("protocol".to_owned(), serde_json::json!("http"));
                if let Some(resolution) = source.get("res") {
                    if let Some(width) = json_i64(resolution, "width") {
                        format.insert("width".to_owned(), serde_json::json!(width));
                    }
                    if let Some(height) = json_i64(resolution, "height") {
                        format.insert("height".to_owned(), serde_json::json!(height));
                    }
                }
                formats.push(serde_json::Value::Object(format));
            }
        }
        if let Some(manifests) = video.get("manifests").and_then(serde_json::Value::as_object) {
            for manifest_type in manifests.keys() {
                if !matches!(manifest_type.as_str(), "hls" | "dash") {
                    return Err(ExtractorError::new(
                        ExtractorErrorKind::Unsupported,
                        format!(
                            "TODO: Canal Alpha native extractor does not implement manifest type {manifest_type}"
                        ),
                    ));
                }
            }
            for (manifest_type, format_id, protocol) in [
                ("hls", "hls", "m3u8_native"),
                ("dash", "dash", "http_dash_segments"),
            ] {
                let Some(manifest_url) = manifests
                    .get(manifest_type)
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                formats.push(serde_json::json!({
                    "url": manifest_url,
                    "format_id": format_id,
                    "ext": "mp4",
                    "protocol": protocol,
                }));
            }
        }
        let first_format = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Canal Alpha video {video_id} has no playable formats"),
            )
        })?;
        let title = json_string(data, "title")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&video_id)
            .to_owned();
        let description = ["longDesc", "shortDesc"].iter().find_map(|key| {
            json_string(data, key)
                .map(html_text_fragment)
                .filter(|value| !value.is_empty())
        });
        let upload_date = ["webPublishAt", "featuredAt", "diffusionDate"]
            .iter()
            .find_map(|key| json_string(data, key))
            .and_then(date_digits);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", json_string(data, "poster"));
        info.insert_if_some("upload_date", upload_date);
        info.insert_if_some("duration", json_i64(video, "duration"));
        info.insert_if_some("url", first_format.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first_format.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
