/// Native Cloudflare Stream manifest URL extractor.
pub struct CloudflareStreamExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CloudflareStreamExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CloudflareStreamExtractor {
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
        _context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Cloudflare Stream URL did not match its native pattern",
            )
        })?;
        let domain = captures
            .name("domain")
            .map(|value| value.as_str())
            .filter(|value| *value != "bytehighway.net")
            .unwrap_or("cloudflarestream.com");
        let matched_id = captures
            .name("id")
            .map(|value| percent_decode(value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Cloudflare Stream URL has no video ID",
                )
            })?;
        let base_url = format!("https://{domain}/{matched_id}/");
        let video_id = if matched_id.contains('.') {
            cloudflare_stream_subject(&matched_id).ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    "TODO: Cloudflare Stream native extractor could not decode the signed video ID",
                )
            })?
        } else {
            matched_id.clone()
        };
        let hls_url = format!("{base_url}manifest/video.m3u8");
        let dash_url = format!("{base_url}manifest/video.mpd");
        let formats = serde_json::json!([
            {
                "format_id": "hls",
                "url": hls_url,
                "ext": "mp4",
                "protocol": "m3u8_native"
            },
            {
                "format_id": "dash",
                "url": dash_url,
                "ext": "mp4",
                "protocol": "http_dash_segments"
            }
        ]);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(video_id));
        info.insert(
            "thumbnail",
            serde_json::json!(format!("{base_url}thumbnails/thumbnail.jpg")),
        );
        info.insert(
            "url",
            formats
                .as_array()
                .and_then(|formats| formats.first())
                .and_then(|format| format.get("url"))
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", formats);
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn cloudflare_stream_subject(token: &str) -> Option<String> {
    let mut sections = token.split('.');
    sections.next()?;
    let payload = sections.next()?;
    let payload = base64url_decode(payload)?;
    let payload: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    json_string(&payload, "sub").map(str::to_owned)
}

fn base64url_decode(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(value.len() * 3 / 4);
    let mut accumulator = 0u32;
    let mut bits = 0u8;
    for character in value.bytes() {
        let digit = match character {
            b'A'..=b'Z' => character - b'A',
            b'a'..=b'z' => character - b'a' + 26,
            b'0'..=b'9' => character - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
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
