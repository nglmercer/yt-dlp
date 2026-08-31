/// Native Newsy page-data/HLS extractor.
pub struct NewsyExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NewsyExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NewsyExtractor {
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
                "Newsy URL did not match its native pattern",
            )
        })?;
        let display_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Newsy URL has no story ID")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let page_data = html_data_json_attribute(&html, "video-player").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Newsy story {display_id} has no video player data"),
            )
        })?;
        let video_id = json_string(&page_data, "id")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Newsy story {display_id} has no video ID"),
                )
            })?;
        let stream_url = json_string(&page_data, "stream")
            .map(|value| proto_relative_url(value, "https:"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Newsy video {video_id} has no HLS stream"),
                )
            })?;
        let json_ld = html_json_ld(&html);
        let title = json_ld
            .as_ref()
            .and_then(newsy_json_ld_object)
            .and_then(|value| newsy_json_string(value, "name"))
            .or_else(|| {
                json_string(&page_data, "headline")
                    .map(unescape_html_attribute)
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or_else(|| display_id.clone());
        let description = json_ld
            .as_ref()
            .and_then(newsy_json_ld_object)
            .and_then(|value| newsy_json_string(value, "description"));
        let thumbnail = json_string(&page_data, "image")
            .map(|value| proto_relative_url(value, "https:"))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                json_ld
                    .as_ref()
                    .and_then(newsy_json_ld_object)
                    .and_then(newsy_json_ld_thumbnail)
            });
        let duration = json_ld
            .as_ref()
            .and_then(newsy_json_ld_object)
            .and_then(|value| {
                json_f64(value, "duration").or_else(|| {
                    json_string(value, "duration").and_then(yt_dlp_core::parse_duration)
                })
            })
            .or_else(|| json_f64(&page_data, "duration"));
        let timestamp = json_ld
            .as_ref()
            .and_then(newsy_json_ld_object)
            .and_then(|value| {
                json_string(value, "uploadDate")
                    .map(str::to_owned)
                    .and_then(parse_timestamp)
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn newsy_json_ld_object(value: &serde_json::Value) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Object(object) => {
            if object.contains_key("contentUrl")
                || object.contains_key("url")
                || object.contains_key("name")
                || object.contains_key("description")
                || object.contains_key("duration")
                || object.contains_key("uploadDate")
            {
                Some(value)
            } else {
                object
                    .get("@graph")
                    .and_then(newsy_json_ld_object)
                    .or_else(|| object.contains_key("name").then_some(value))
            }
        }
        serde_json::Value::Array(values) => values.iter().find_map(newsy_json_ld_object),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => None,
    }
}

fn newsy_json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    json_string(value, key)
        .map(unescape_html_attribute)
        .filter(|value| !value.is_empty())
}

fn newsy_json_ld_thumbnail(value: &serde_json::Value) -> Option<String> {
    ["thumbnailUrl", "thumbnailURL", "thumbnail_url"]
        .iter()
        .find_map(|key| value.get(*key).and_then(newsy_thumbnail_value))
        .map(|value| proto_relative_url(&value, "https:"))
}

fn newsy_thumbnail_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(unescape_html_attribute(value)),
        serde_json::Value::Array(values) => values.iter().find_map(newsy_thumbnail_value),
        serde_json::Value::Object(_) => json_string(value, "url")
            .or_else(|| json_string(value, "contentUrl"))
            .map(unescape_html_attribute),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_) => None,
    }
}
