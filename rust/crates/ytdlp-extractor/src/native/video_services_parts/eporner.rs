/// Native Eporner page/XHR media extractor.
pub struct EpornerExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EpornerExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EpornerExtractor {
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
        let initial_captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Eporner URL did not match its native pattern",
            )
        })?;
        let initial_id = initial_captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Eporner URL has no video ID")
            })?;
        let display_id = initial_captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| initial_id.clone());
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let video_id = self
            .matcher
            .captures(response.url())
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .unwrap_or(initial_id);
        let video_hash = eporner_capture(&webpage, r#"(?is)hash\s*[:=]\s*["']([\da-f]{32})"#)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Eporner video {video_id} has no playback hash"),
                )
            })?;
        let encoded_hash = eporner_base36_hash(&video_hash).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Eporner video {video_id} has an invalid playback hash"),
            )
        })?;
        let mut request = Request::new(&format!("http://www.eporner.com/xhr/video/{video_id}"));
        request.update_query(&[
            ("hash".to_owned(), encoded_hash),
            ("device".to_owned(), "generic".to_owned()),
            ("domain".to_owned(), "www.eporner.com".to_owned()),
            ("fallback".to_owned(), "false".to_owned()),
        ]);
        let response = context.request(&request)?;
        let video = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Eporner video JSON for {video_id}: {error}"),
            )
        })?;
        if json_bool(&video, "available") == Some(false) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!(
                    "Eporner said: {}",
                    json_string(&video, "message").unwrap_or("video unavailable")
                ),
            ));
        }
        let has_av1 = Regex::new(r#"(?is)class\s*=\s*["'][^"']*\bdownload-av1\b"#)
            .ok()
            .and_then(|matcher| matcher.find(&webpage).ok().flatten())
            .is_some();
        let formats = eporner_formats(video.get("sources"), has_av1);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Eporner video {video_id} has no playable sources"),
            ));
        }
        let json_ld = html_json_ld(&webpage).unwrap_or(serde_json::Value::Null);
        let title = html_meta_value(&webpage, "og:title")
            .or_else(|| eporner_capture(&webpage, r#"(?is)<title[^>]*>(.*?)\s*-\s*EPORNER"#))
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                json_string(&json_ld, "name")
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| display_id.clone());
        let duration = html_meta_value(&webpage, "duration")
            .and_then(|value| yt_dlp_core::parse_duration(&value));
        let view_count = eporner_capture(
            &webpage,
            r#"(?is)id\s*=\s*["']cinemaviews1["'][^>]*>\s*([0-9,\s]+)"#,
        )
        .and_then(|value| value.replace([',', ' '], "").parse::<i64>().ok());
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            json_string(&json_ld, "description").map(str::to_owned),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&json_ld, "thumbnailUrl").map(str::to_owned),
        );
        info.insert_if_some("duration", duration);
        info.insert_if_some("view_count", view_count);
        info.insert("age_limit", serde_json::json!(18));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn eporner_capture(html: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn eporner_base36_hash(value: &str) -> Option<String> {
    if value.len() != 32 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }
    let mut encoded = String::new();
    for chunk in value.as_bytes().chunks_exact(8) {
        let chunk = std::str::from_utf8(chunk).ok()?;
        let number = u32::from_str_radix(chunk, 16).ok()?;
        encoded.push_str(&eporner_base36(number));
    }
    Some(encoded)
}

fn eporner_base36(mut value: u32) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_owned();
    }
    let mut digits = Vec::new();
    while value > 0 {
        digits.push(DIGITS[(value % 36) as usize] as char);
        value /= 36;
    }
    digits.into_iter().rev().collect()
}

fn eporner_formats(
    sources: Option<&serde_json::Value>,
    has_av1: bool,
) -> Vec<serde_json::Value> {
    let Some(sources) = sources.and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    let mut formats = Vec::new();
    for (kind, source_group) in sources {
        let Some(source_group) = source_group.as_object() else {
            continue;
        };
        for (format_id, source) in source_group {
            let Some(media_url) = json_string(source, "src")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            if kind == "hls" {
                formats.push(serde_json::json!({
                    "url": media_url,
                    "format_id": "hls",
                    "protocol": "m3u8_native",
                    "ext": "mp4",
                }));
                continue;
            }
            let height = eporner_capture(format_id, r"(?i)(\d+)[pP]")
                .and_then(|value| value.parse::<i64>().ok());
            let fps = eporner_capture(format_id, r"(?i)(\d+)fps")
                .and_then(|value| value.parse::<i64>().ok());
            let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "protocol": "http",
                "ext": extension,
            });
            if let Some(height) = height {
                format["height"] = serde_json::json!(height);
            }
            if let Some(fps) = fps {
                format["fps"] = serde_json::json!(fps);
            }
            formats.push(format);
            if has_av1 {
                let av1_url = media_url.replace(".mp4", "-av1.mp4");
                let mut av1_format = serde_json::json!({
                    "url": av1_url,
                    "format_id": format!("av1-{format_id}"),
                    "protocol": "http",
                    "ext": "mp4",
                    "vcodec": "av1",
                });
                if let Some(height) = height {
                    av1_format["height"] = serde_json::json!(height);
                }
                if let Some(fps) = fps {
                    av1_format["fps"] = serde_json::json!(fps);
                }
                formats.push(av1_format);
            }
        }
    }
    formats
}
