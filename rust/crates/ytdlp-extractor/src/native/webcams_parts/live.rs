/// Native Skyline Webcams live HLS extractor.
pub struct SkylineWebcamsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SkylineWebcamsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SkylineWebcamsExtractor {
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
                    "Skyline Webcams URL has no ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = Regex::new(
            r#"(?is)(?:\burl|\bsource)\s*:\s*["']((?:https?:)?//[^"']+?\.m3u8[^"']*)["']"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| proto_relative_url(value.as_str(), "https:"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Skyline Webcams stream {video_id} has no HLS URL"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title")
                    .or_else(|| html_title_value(&html))
                    .unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Webcamera.pl live extractor. The service obfuscates its HLS URL
/// with ROT13 in the page, which is decoded locally in Rust.
pub struct WebcameraplExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WebcameraplExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WebcameraplExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Webcamera.pl URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let encoded_url = Regex::new(r#"(?is)\bdata-src\s*=\s*"([^"]+\.z3h8)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Webcamera.pl stream {video_id} has no encoded HLS URL"),
                )
            })?;
        let media_url = rot13_ascii(&encoded_url);
        let title = Regex::new(r#"(?is)<h1\b[^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| video_id.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Alibaba product video extractor. Product pages expose their media
/// records in the detailData object; the selected video is returned directly.
pub struct AlibabaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AlibabaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AlibabaExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Alibaba URL has no product ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let detail = json_object_after_marker(&html, "window.detailData").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Alibaba product {display_id} has no detailData"),
            )
        })?;
        let product = detail
            .get("globalData")
            .and_then(|value| value.get("product"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Alibaba product {display_id} has no media product"),
                )
            })?;
        let media = product
            .get("mediaItems")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| {
                items.iter().find(|item| {
                    json_string(item, "type") == Some("video")
                        && item.get("videoId").is_some()
                        && json_string(item, "videoUrl").is_some()
                })
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Alibaba product {display_id} has no playable video"),
                )
            })?;
        let video_id = json_value_string(media.get("videoId")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Alibaba video record has no video ID",
            )
        })?;
        let media_url = json_string(media, "videoUrl").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Alibaba video record has no video URL",
            )
        })?;
        let ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let mut format = serde_json::json!({
            "url": media_url,
            "format_id": json_string(media, "definition").unwrap_or("source"),
            "ext": ext,
        });
        for (source, target) in [
            ("bitrate", "tbr"),
            ("width", "width"),
            ("height", "height"),
            ("length", "filesize"),
        ] {
            if let Some(value) = json_i64(media, source) {
                format[target] = serde_json::json!(value);
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(product, "subject"));
        info.insert_if_some("duration", json_f64(media, "duration"));
        info.insert_if_some("thumbnail", json_string(media, "videoCoverUrl"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext));
        info.insert("formats", serde_json::json!([format]));
        Ok(ExtractorResult::single(info))
    }
}
