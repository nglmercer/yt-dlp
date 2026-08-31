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

/// Native Moving Image archive extractor. Archive pages expose one HLS
/// manifest and a small set of labelled metadata fields.
pub struct MovingImageExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MovingImageExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MovingImageExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Moving Image URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = Regex::new(r#"(?is)\bfile\s*:\s*"([^"]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Moving Image film {video_id} has no HLS URL"),
                )
            })?;
        let title = html_field_value(&html, "Title")
            .map(|value| value.trim_matches(['(', ')', '[', ']']).trim().to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| html_title_value(&html))
            .unwrap_or_else(|| video_id.clone());
        let description = html_field_value(&html, "Description");
        let duration = html_field_value(&html, "Running time").and_then(|value| {
            yt_dlp_core::parse_duration(value.trim_matches(['(', ')', '[', ']']))
        });
        let thumbnail = Regex::new(r#"(?is)\bimage\s*:\s*'([^']+)'"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, value.as_str()));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Tweakers video API extractor.
pub struct TweakersExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl TweakersExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for TweakersExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Tweakers URL has no ID")
            })?;
        let data = context.get_json(&format!(
            "https://tweakers.net/video/s1playlist/{video_id}/1920/1080/playlist.json"
        ))?;
        let item = data
            .get("items")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Tweakers video {video_id} has no API item"),
                )
            })?;
        let mut formats = Vec::new();
        if let Some(locations) = item
            .get("locations")
            .and_then(|value| value.get("progressive"))
            .and_then(serde_json::Value::as_array)
        {
            for location in locations {
                let format_id = json_string(location, "label");
                for source in location
                    .get("sources")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(media_url) = json_string(source, "src") else {
                        continue;
                    };
                    let ext = json_string(source, "type")
                        .and_then(|value| mimetype_extension(value.split(';').next()))
                        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(media_url), "mp4"));
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": format_id,
                        "ext": ext,
                    });
                    if let Some(value) = json_i64(location, "width") {
                        format["width"] = serde_json::json!(value);
                    }
                    if let Some(value) = json_i64(location, "height") {
                        format["height"] = serde_json::json!(value);
                    }
                    formats.push(format);
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Tweakers video {video_id} has no progressive formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(item, "title"));
        info.insert_if_some("description", json_string(item, "description"));
        info.insert_if_some("thumbnail", json_string(item, "poster"));
        info.insert_if_some("duration", json_i64(item, "duration"));
        info.insert_if_some("uploader_id", json_string(item, "account"));
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
        Ok(ExtractorResult::single(info))
    }
}

/// Native KrasView page extractor.
pub struct KrasViewExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KrasViewExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KrasViewExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "KrasView URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let flashvars = json_object_after_marker(&html, "video_Init(").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KrasView video {video_id} has no player data"),
            )
        })?;
        let media_url = json_string(&flashvars, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KrasView video {video_id} has no media URL"),
            )
        })?;
        let ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_title_value(&html)),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some(
            "thumbnail",
            json_string(&flashvars, "image")
                .map(str::to_owned)
                .or_else(|| html_meta_value(&html, "og:image")),
        );
        info.insert_if_some("duration", json_i64(&flashvars, "duration"));
        info.insert_if_some(
            "width",
            html_meta_value(&html, "video:width").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert_if_some(
            "height",
            html_meta_value(&html, "video:height").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": ext,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native 56.com page/API extractor. The legacy Sohu redirect variant is
/// surfaced as an explicit TODO because its target extractor is not yet
/// native; the direct XML API path is fully handled here.
pub struct C56Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl C56Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for C56Extractor {
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
                "56.com URL did not match its native pattern",
            )
        })?;
        let text_id = captures
            .name("textid")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "56.com URL has no text ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        if let Some(sohu_info) = json_object_after_marker(&html, "var sohuVideoInfo") {
            if let Some(sohu_url) = json_string(&sohu_info, "url") {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: 56.com Sohu wrapper requires native Sohu extraction ({sohu_url})"
                    ),
                ));
            }
        }
        let page = context.get_json(&format!("http://vxml.56.com/json/{text_id}/"))?;
        let info_data = page.get("info").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("56.com API response for {text_id} has no info"),
            )
        })?;
        let video_id = json_value_string(info_data.get("vid")).unwrap_or_else(|| text_id.clone());
        let mut formats = Vec::new();
        for file in info_data
            .get("rfiles")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(media_url) = json_string(file, "url") else {
                continue;
            };
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": json_string(file, "type").unwrap_or("source"),
                "ext": yt_dlp_core::determine_ext(Some(media_url), "flv"),
            });
            if let Some(value) = json_i64(file, "filesize") {
                format["filesize"] = serde_json::json!(value);
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("56.com video {video_id} has no media files"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let duration = json_f64(info_data, "duration").map(|value| value / 1000.0);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(info_data, "Subject"));
        info.insert_if_some("duration", duration);
        info.insert_if_some(
            "thumbnail",
            json_string(info_data, "bimg").or_else(|| json_string(info_data, "img")),
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
                .unwrap_or_else(|| serde_json::json!("flv")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native TASS page extractor. JW-style source records embedded in the page
/// are parsed as JSON data and filtered to HTTP MP4 renditions.
pub struct TassExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl TassExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for TassExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "TASS URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let sources = json_array_after_marker(&html, "sources").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("TASS video {video_id} has no source list"),
            )
        })?;
        let mut formats = Vec::new();
        for source in sources.as_array().into_iter().flatten() {
            let Some(media_url) = json_string(source, "file") else {
                continue;
            };
            if !media_url.starts_with("http") || !media_url.ends_with(".mp4") {
                continue;
            }
            let format_id = json_string(source, "label").unwrap_or("source");
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "ext": "mp4",
                "quality": if format_id == "hd" { 1 } else { 0 },
            });
            if let Some(value) = json_i64(source, "width") {
                format["width"] = serde_json::json!(value);
            }
            if let Some(value) = json_i64(source, "height") {
                format["height"] = serde_json::json!(value);
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("TASS video {video_id} has no HTTP MP4 sources"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_title_value(&html)),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Photobucket page/API extractor.
pub struct PhotobucketExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PhotobucketExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PhotobucketExtractor {
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
                "Photobucket URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Photobucket URL has no ID")
            })?;
        let extension = captures
            .name("ext")
            .map(|value| value.as_str().to_ascii_lowercase())
            .unwrap_or_else(|| "mp4".to_owned());
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let data = json_object_after_marker(&html, "Pb.Data.Shared.MEDIA").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Photobucket media {video_id} has no shared metadata"),
            )
        })?;
        let html_code = data
            .get("linkcodes")
            .and_then(|value| json_string(value, "html"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Photobucket media {video_id} has no HTML link code"),
                )
            })?;
        let media_url = Regex::new(r#"(?is)\bfile=([^&\s]+?\.mp4)"#)
            .ok()
            .and_then(|matcher| matcher.captures(html_code).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, &percent_decode(value.as_str())))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Photobucket media {video_id} has no file URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert_if_some("uploader", json_string(&data, "username"));
        info.insert_if_some("timestamp", json_i64(&data, "creationDate"));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert_if_some("thumbnail", json_string(&data, "thumbUrl"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": extension,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Nobel Prize media-page extractor. Video JSON-LD and metadata are
/// read directly; query aliases id and qid are both supported.
pub struct NobelPrizeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NobelPrizeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NobelPrizeExtractor {
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
        if !self.suitable(url) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Nobel Prize URL did not match its native pattern",
            ));
        }
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Nobel Prize URL: {error}"),
            )
        })?;
        let video_id = parsed
            .query_pairs()
            .find(|(key, _)| key == "id" || key == "qid")
            .map(|(_, value)| value.into_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Nobel Prize URL requires id or qid",
                )
            })?;
        let page_url = format!(
            "https://mediaplayer.nobelprize.org{}",
            parsed
                .path()
                .is_empty()
                .then_some("/mediaplayer/")
                .unwrap_or(parsed.path())
        );
        let webpage = context.get(&page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let data = html_json_ld(&html).unwrap_or(serde_json::Value::Null);
        let media_url = json_string(&data, "contentUrl")
            .or_else(|| json_string(&data, "url"))
            .map(str::to_owned)
            .or_else(|| html_meta_value(&html, "contentUrl"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Nobel Prize media {video_id} has no content URL"),
                )
            })?;
        let media_url = proto_relative_url(&media_url, "https:");
        let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "caption")
                    .or_else(|| json_string(&data, "name").map(str::to_owned))
                    .unwrap_or(video_id.clone())
            ),
        );
        info.insert_if_some(
            "description",
            json_string(&data, "description")
                .map(str::to_owned)
                .or_else(|| html_meta_value(&html, "description")),
        );
        info.insert_if_some("thumbnail", json_string(&data, "thumbnailUrl"));
        info.insert_if_some(
            "duration",
            json_string(&data, "duration").and_then(yt_dlp_core::parse_duration),
        );
        info.insert_if_some(
            "timestamp",
            json_string(&data, "uploadDate")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": ext,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Caltrans traffic-camera live HLS extractor.
pub struct CaltransExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CaltransExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CaltransExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Caltrans URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = Regex::new(r#"(?is)\bvideoStreamURL\s*=\s*"([^"]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| proto_relative_url(value.as_str(), "https:"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Caltrans camera {video_id} has no stream URL"),
                )
            })?;
        let route_place = Regex::new(r#"(?is)\broutePlace\s*=\s*"([^"]*)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned());
        let location = Regex::new(r#"(?is)\blocationName\s*=\s*"([^"]*)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let title = route_place
            .map(|place| format!("{place} : {location}"))
            .unwrap_or(location);
        let thumbnail = Regex::new(r#"(?is)\bposterURL\s*=\s*"([^"]*)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| proto_relative_url(value.as_str(), "https:"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("ts"));
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "ts",
                "protocol": "m3u8_native",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
