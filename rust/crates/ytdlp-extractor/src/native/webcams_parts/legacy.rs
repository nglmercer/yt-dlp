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
