/// Native InfoQ presentation extractor.
///
/// HTTP video/audio renditions are fully represented here. Legacy RTMP
/// renditions remain in the result for parity and are rejected later by the
/// native downloader's explicit unsupported-protocol TODO.
pub struct InfoqExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl InfoqExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for InfoqExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "InfoQ URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let auth = infoq_cloudfront_auth(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("InfoQ presentation {video_id} has no CloudFront auth"),
            )
        })?;
        let mut formats = Vec::new();
        if let Some(encoded_id) = infoq_assignment(&webpage, "jsclassref") {
            if let Some(real_id) = infoq_base64_decode(&encoded_id)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .map(|value| percent_decode(&value))
            {
                formats.push(serde_json::json!({
                    "format_id": "rtmp_video",
                    "url": "rtmpe://videof.infoq.com/cfx/st/",
                    "ext": "mp4",
                    "play_path": format!("mp4:{real_id}"),
                }));
            }
        }
        if let Some(video_url) = infoq_assignment(&webpage, "P.s")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            let video_url = infoq_add_auth(&video_url, &auth);
            formats.push(serde_json::json!({
                "format_id": "http_video",
                "url": video_url,
                "ext": yt_dlp_core::determine_ext(Some(&video_url), "mp4"),
                "http_headers": {"Referer": "https://www.infoq.com/"},
            }));
        }
        if let Some(filename) = infoq_hidden_filename(&webpage) {
            let audio_url = infoq_add_auth(
                &resolve_url("http://ress.infoq.com/downloads/mp3downloads/", &filename),
                &auth,
            );
            formats.push(serde_json::json!({
                "format_id": "http_audio",
                "url": audio_url,
                "ext": "mp3",
                "vcodec": "none",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("InfoQ presentation {video_id} has no media formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", html_title_value(&webpage));
        info.insert_if_some("description", html_meta_value(&webpage, "description"));
        info.insert("url", first.get("url").cloned().unwrap_or(serde_json::Value::Null));
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn infoq_assignment(webpage: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is){}\s*=\s*['"]([^'"]*)"#,
        regex::escape(name)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
        .filter(|value| !value.trim().is_empty())
}

fn infoq_cloudfront_auth(webpage: &str) -> Option<[String; 3]> {
    Some([
        infoq_constant(webpage, "scp")?,
        infoq_constant(webpage, "scs")?,
        infoq_constant(webpage, "sck")?,
    ])
}

fn infoq_constant(webpage: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)\bInfoQConstants\.{}\s*=\s*['"]([^'"]*)"#,
        regex::escape(name)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
}

fn infoq_add_auth(value: &str, auth: &[String; 3]) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return value.to_owned();
    };
    url.query_pairs_mut()
        .append_pair("Policy", &auth[0])
        .append_pair("Signature", &auth[1])
        .append_pair("Key-Pair-Id", &auth[2]);
    url.to_string()
}

fn infoq_hidden_filename(webpage: &str) -> Option<String> {
    let form = html_element_by_id(webpage, "mp3Form")?;
    html_named_input_value(&form, "filename")
}

fn infoq_base64_decode(value: &str) -> Option<Vec<u8>> {
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
