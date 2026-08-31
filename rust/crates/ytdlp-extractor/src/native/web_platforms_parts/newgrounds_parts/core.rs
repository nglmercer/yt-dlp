/// Native APA embed/player extractor. JWPlatform-backed pages return an
/// explicit native redirect; older players expose direct HLS/progressive URLs.
pub struct ApaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ApaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ApaExtractor {
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
                "APA URL did not match its native pattern",
            )
        })?;
        let base_url = captures
            .name("base_url")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "APA URL has no base URL")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "APA URL has no ID")
            })?;
        let player_url = format!("{base_url}/player/{video_id}");
        let webpage = context.get(&player_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let field = |name: &str| {
            let pattern = format!(r#"(?is)\b{}\s*:\s*["']([^"']+)["']"#, regex::escape(name));
            Regex::new(&pattern)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
        };
        if let Some(jwplatform_id) = Regex::new(r#"(?i)\bmedia[iI]d\s*:\s*["']([a-zA-Z0-9]{8})"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
        {
            return Ok(ExtractorResult::Redirect {
                url: format!("jwplatform:{jwplatform_id}"),
                ie_key: Some("JWPlatform".to_owned()),
            });
        }
        let title = field("title").unwrap_or_else(|| video_id.clone());
        let mut formats = Vec::new();
        if let Some(source_url) = field("hls").or_else(|| field("hlsUrl")) {
            formats.push(serde_json::json!({
                "url": source_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        if let Some(source_url) = field("progressive") {
            let height = Regex::new(r#"(?i)(\d+)\.mp4(?:$|[?#])"#)
                .ok()
                .and_then(|matcher| matcher.captures(&source_url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<i64>().ok());
            formats.push(serde_json::json!({
                "url": source_url,
                "format_id": "progressive",
                "height": height,
                "ext": "mp4",
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("APA video {video_id} has no playable sources"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", field("description"));
        info.insert_if_some("thumbnail", field("poster"));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native href.li redirect extractor. URL results are represented explicitly
/// so the Rust CLI can follow them without a compatibility runtime.
pub struct HrefLiRedirectExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HrefLiRedirectExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HrefLiRedirectExtractor {
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
                "href.li URL did not match its native pattern",
            )
        })?;
        let target = captures
            .name("url")
            .map(|value| percent_decode(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "href.li URL has no target")
            })?;
        Ok(ExtractorResult::Redirect {
            url: target,
            ie_key: None,
        })
    }
}

/// Native Streamable AJAX extractor. Streamable's public API exposes the
/// complete media inventory, including the older records that do not have
/// video dimensions or codec metadata, so no browser runtime is needed.
pub struct StreamableExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl StreamableExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for StreamableExtractor {
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
                "Streamable URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Streamable URL has no ID")
            })?;
        let video = context.get_json(&format!("https://ajax.streamable.com/videos/{video_id}"))?;
        if json_i64(&video, "status") != Some(2) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Streamable video {video_id} is unavailable or still processing"),
            ));
        }

        let files = video
            .get("files")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Streamable video {video_id} has no media files"),
                )
            })?;
        let mut formats = Vec::new();
        for (format_id, file) in files {
            let Some(raw_url) = json_string(file, "url") else {
                continue;
            };
            let media_url = proto_relative_url(raw_url, "https:");
            let mut format = serde_json::Map::new();
            format.insert("format_id".to_owned(), serde_json::json!(format_id));
            format.insert("url".to_owned(), serde_json::json!(media_url));
            format.insert(
                "ext".to_owned(),
                serde_json::json!(yt_dlp_core::determine_ext(Some(raw_url), "mp4")),
            );
            if let Some(value) = json_i64(file, "width") {
                format.insert("width".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_i64(file, "height") {
                format.insert("height".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_i64(file, "size") {
                format.insert("filesize".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_i64(file, "framerate") {
                format.insert("fps".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_f64(file, "bitrate") {
                format.insert("vbr".to_owned(), serde_json::json!(value / 1000.0));
            }
            if let Some(metadata) = file.get("input_metadata") {
                if let Some(value) = json_string(metadata, "video_codec_name") {
                    format.insert("vcodec".to_owned(), serde_json::json!(value));
                }
                if let Some(value) = json_string(metadata, "audio_codec_name") {
                    format.insert("acodec".to_owned(), serde_json::json!(value));
                }
            }
            formats.push(serde_json::Value::Object(format));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Streamable video {video_id} has no playable media files"),
            ));
        }

        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                json_string(&video, "reddit_title")
                    .or_else(|| json_string(&video, "title"))
                    .unwrap_or(video_id)
            ),
        );
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("description", json_string(&video, "description"));
        info.insert_if_some(
            "thumbnail",
            json_string(&video, "thumbnail_url").map(|value| proto_relative_url(value, "https:")),
        );
        info.insert_if_some(
            "uploader",
            video
                .get("owner")
                .and_then(|owner| json_string(owner, "user_name")),
        );
        info.insert_if_some("timestamp", json_f64(&video, "date_added"));
        info.insert_if_some("duration", json_f64(&video, "duration"));
        info.insert_if_some("view_count", json_i64(&video, "plays"));
        Ok(ExtractorResult::single(info))
    }
}
