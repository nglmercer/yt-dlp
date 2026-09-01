/// Native Dropbox streamed-prefetch extractor.
pub struct DropboxExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DropboxExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DropboxExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Dropbox URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let encoded_parts = Regex::new(
            r#"(?is)registerStreamedPrefetch\s*\(\s*"[\w/+=]+"\s*,\s*"([\w/+=]+)"#,
        )
        .ok()
        .map(|matcher| {
            matcher
                .captures_iter(&webpage)
                .flatten()
                .filter_map(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
        if encoded_parts.is_empty() {
            return Err(dropbox_todo(
                &video_id,
                "the page has no registerStreamedPrefetch payloads",
            ));
        }
        let mut parts = Vec::new();
        for encoded in encoded_parts.into_iter().rev() {
            let decoded = dropbox_base64_decode(&encoded).ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Dropbox share {video_id} has invalid prefetch data"),
                )
            })?;
            parts.push(String::from_utf8_lossy(&decoded).into_owned());
        }
        if parts.iter().any(|part| part.contains("/sm/password")) {
            return Err(dropbox_todo(
                &video_id,
                "password-protected shares require the native video-password option",
            ));
        }
        let mut formats = Vec::new();
        let mut thumbnail = None;
        let mut has_anonymous_download = false;
        let hls_matcher = Regex::new(r#"(?i)https://[^\s"'<>]+\.m3u8(?:\?[^\s"'<>]*)?"#).map_err(
            |error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Dropbox HLS matcher: {error}"),
                )
            },
        )?;
        let thumbnail_matcher = Regex::new(
            r#"(?i)https://www\.dropbox\.com/temp_thumb_from_token/[\w/?&=]+"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Dropbox thumbnail matcher: {error}"),
            )
        })?;
        for part in parts {
            has_anonymous_download |= part.contains("anonymous:\tanonymous");
            if formats.is_empty() {
                if let Some(media_url) = hls_matcher.find(&part).ok().flatten().map(|value| value.as_str().to_owned()) {
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": "hls",
                        "protocol": "m3u8_native",
                        "ext": "mp4",
                    }));
                    thumbnail = thumbnail_matcher
                        .find(&part)
                        .ok()
                        .flatten()
                        .map(|value| value.as_str().to_owned());
                }
            }
        }
        if has_anonymous_download {
            let original_url = dropbox_download_url(url, &video_id)?;
            formats.push(serde_json::json!({
                "url": original_url,
                "format_id": "original",
                "format_note": "Original",
                "quality": 1,
                "ext": yt_dlp_core::determine_ext(Some(url), "mp4"),
            }));
        }
        if formats.is_empty() {
            return Err(dropbox_todo(
                &video_id,
                "the decoded page contains no supported HLS or anonymous-original stream",
            ));
        }
        let title = last_path_segment(url)
            .map(|value| percent_decode(&value))
            .unwrap_or_else(|_| video_id.clone())
            .split_once('.')
            .map_or_else(
                || {
                    last_path_segment(url)
                        .map(|value| percent_decode(&value))
                        .unwrap_or_else(|_| video_id.clone())
                },
                |(title, _)| title.to_owned(),
            );
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
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
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn dropbox_base64_decode(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(value.len() * 3 / 4);
    let mut accumulator = 0u32;
    let mut bits = 0u8;
    for character in value.bytes() {
        let digit = match character {
            b'A'..=b'Z' => character - b'A',
            b'a'..=b'z' => character - b'a' + 26,
            b'0'..=b'9' => character - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        accumulator = (accumulator << 6) | u32::from(digit);
        bits = bits.saturating_add(6);
        while bits >= 8 {
            bits -= 8;
            output.push((accumulator >> bits) as u8);
            if bits > 0 {
                accumulator &= (1u32 << bits) - 1;
            } else {
                accumulator = 0;
            }
        }
    }
    Some(output)
}

fn dropbox_download_url(url: &str, video_id: &str) -> Result<String, ExtractorError> {
    let mut parsed = url::Url::parse(url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Dropbox URL for {video_id}: {error}"),
        )
    })?;
    let existing = parsed
        .query_pairs()
        .filter(|(key, _)| key != "dl")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in existing {
        query.append_pair(&key, &value);
    }
    query.append_pair("dl", "1");
    parsed.set_query(Some(&query.finish()));
    Ok(parsed.to_string())
}

fn dropbox_todo(video_id: &str, detail: &str) -> ExtractorError {
    ExtractorError::new(
        ExtractorErrorKind::Unsupported,
        format!("TODO: Dropbox share {video_id} is not fully supported in Rust: {detail}"),
    )
}
