/// Native Ixigua SSR-hydrated media extractor.
pub struct IxiguaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl IxiguaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for IxiguaExtractor {
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
        let video_id = ixigua_video_id(url).ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Ixigua URL has no video ID")
        })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let script = html_element_by_id(&webpage, "SSR_HYDRATED_DATA").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Ixigua video {video_id} has no SSR_HYDRATED_DATA"),
            )
        })?;
        let script = script.trim();
        let script = script
            .strip_prefix("window._SSR_HYDRATED_DATA=")
            .unwrap_or(script)
            .trim()
            .trim_end_matches(';')
            .trim();
        let hydrated = parse_common_javascript_value(script).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Ixigua video {video_id} has invalid SSR JSON"),
            )
        })?;
        let video = hydrated
            .get("anyVideo")
            .and_then(|value| value.get("gidInformation"))
            .and_then(|value| value.get("packerData"))
            .and_then(|value| value.get("video"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Ixigua video {video_id} has no video data"),
                )
            })?;
        let video_resource = video
            .get("videoResource")
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Ixigua video {video_id} has no media resource"),
                )
            })?;
        let mut formats = Vec::new();
        ixigua_collect_media(
            video_resource.get("video_list"),
            &mut formats,
            IxiguaMediaKind::Video,
        );
        if let Some(dynamic_video) = video_resource.get("dynamic_video") {
            ixigua_collect_media(
                dynamic_video.get("dynamic_video_list"),
                &mut formats,
                IxiguaMediaKind::DynamicVideo,
            );
            ixigua_collect_media(
                dynamic_video.get("dynamic_audio_list"),
                &mut formats,
                IxiguaMediaKind::DynamicAudio,
            );
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Ixigua video {video_id} has no decodable media URLs"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(video, "title"));
        info.insert_if_some("description", json_string(video, "video_abstract"));
        info.insert("url", first.get("url").cloned().unwrap_or(serde_json::Value::Null));
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("like_count", json_i64(video, "video_like_count"));
        info.insert_if_some("dislike_count", json_i64(video, "video_unlike_count"));
        info.insert_if_some("view_count", json_i64(video, "video_watch_count"));
        info.insert_if_some("duration", json_i64(video, "duration"));
        info.insert_if_some("timestamp", json_i64(video, "video_publish_time"));
        info.insert_if_some(
            "uploader_id",
            video
                .get("user_info")
                .and_then(|user| json_value_string(user.get("user_id"))),
        );
        info.insert_if_some(
            "uploader",
            video
                .get("user_info")
                .and_then(|user| json_string(user, "name")),
        );
        info.insert_if_some(
            "tags",
            json_string(video, "tag").map(|tag| serde_json::json!([tag])),
        );
        info.insert_if_some(
            "thumbnail",
            ixigua_find_string(video, &["thumbnail", "video_cover", "cover_url", "poster"]),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

#[derive(Clone, Copy)]
enum IxiguaMediaKind {
    Video,
    DynamicVideo,
    DynamicAudio,
}

fn ixigua_video_id(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let mut segments = parsed.path_segments()?;
    let first = segments.next()?;
    let id = if first == "video" {
        segments.next()?
    } else {
        first
    };
    (!id.is_empty() && id.chars().all(|value| value.is_ascii_digit())).then(|| id.to_owned())
}

fn ixigua_collect_media(
    value: Option<&serde_json::Value>,
    formats: &mut Vec<serde_json::Value>,
    kind: IxiguaMediaKind,
) {
    for media in value.into_iter().flat_map(json_object_values) {
        let Some(encoded_url) = json_string(media, "main_url") else {
            continue;
        };
        let Some(media_url) = ixigua_base64_decode(encoded_url)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
        else {
            continue;
        };
        let mut format = serde_json::json!({
            "url": media_url,
            "ext": if matches!(kind, IxiguaMediaKind::DynamicAudio) { "m4a" } else { "mp4" },
        });
        if let Some(value) = json_i64(media, "vwidth") {
            format["width"] = serde_json::json!(value);
        }
        if let Some(value) = json_i64(media, "vheight") {
            format["height"] = serde_json::json!(value);
        }
        if let Some(value) = json_i64(media, "fps") {
            format["fps"] = serde_json::json!(value);
        }
        if let Some(value) = json_i64(media, "size") {
            format["filesize"] = serde_json::json!(value);
        }
        if let Some(value) = json_string(media, "codec_type") {
            format["vcodec"] = serde_json::json!(value);
        }
        if let Some(value) = json_value_string(media.get("quality_type")) {
            format["format_id"] = serde_json::json!(value);
        }
        if matches!(kind, IxiguaMediaKind::DynamicVideo) {
            format["acodec"] = serde_json::json!("none");
        } else if matches!(kind, IxiguaMediaKind::DynamicAudio) {
            format["vcodec"] = serde_json::json!("none");
        }
        formats.push(format);
    }
}

fn ixigua_find_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    match value {
        serde_json::Value::Object(values) => {
            for key in keys {
                if let Some(value) = values.get(*key).and_then(serde_json::Value::as_str) {
                    return Some(value.to_owned());
                }
            }
            values.values().find_map(|value| ixigua_find_string(value, keys))
        }
        serde_json::Value::Array(values) => values.iter().find_map(|value| ixigua_find_string(value, keys)),
        _ => None,
    }
}

fn ixigua_base64_decode(value: &str) -> Option<Vec<u8>> {
    let mut accumulator = 0u32;
    let mut bit_count = 0u8;
    let mut decoded = Vec::with_capacity(value.len() * 3 / 4);
    for byte in value.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            break;
        }
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        accumulator = (accumulator << 6) | u32::from(digit);
        bit_count = bit_count.saturating_add(6);
        if bit_count >= 8 {
            bit_count -= 8;
            decoded.push(((accumulator >> bit_count) & 0xff) as u8);
        }
    }
    Some(decoded)
}
