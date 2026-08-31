/// Native NonkTube HTML5-video extractor.
pub struct NonkTubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NonkTubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NonkTubeExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "NonkTube URL has no video ID")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let title = html_meta_value(&html, "og:title")
            .map(|value| unescape_html_attribute(&value))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("NonkTube video {video_id} has no title"),
                )
            })?;
        let formats = html5_media_formats(url, &html);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("NonkTube video {video_id} has no HTML5 media"),
            ));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("NonkTube video {video_id} has no usable media"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("age_limit", serde_json::json!(18));
        info.insert_if_some("thumbnail", nuevo_html5_thumbnail(url, &html));
        info.insert_if_some(
            "duration",
            html_meta_value(&html, "og:video:duration")
                .and_then(|value| value.parse::<f64>().ok())
                .or_else(|| nuevo_html5_duration(&html)),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

/// Native LoveHomePorn extractor backed by the shared Nuevo XML config.
pub struct LoveHomePornExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LoveHomePornExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LoveHomePornExtractor {
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
                "LoveHomePorn URL did not match its native pattern",
            )
        })?;
        let requested_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "LoveHomePorn URL has no ID")
            })?;
        let display_id = captures.name("display_id").map(|value| value.as_str().to_owned());
        let config_url =
            format!("http://lovehomeporn.com/media/nuevo/config.php?key={requested_id}");
        let response = context.get(&config_url)?;
        let xml = String::from_utf8_lossy(response.body());
        let title = xml_element_text(&xml, "title")
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("LoveHomePorn video {requested_id} has no title"),
                )
            })?;
        let video_id = xml_element_text(&xml, "mediaid")
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| requested_id.clone());
        let formats = nuevo_xml_formats(&xml);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LoveHomePorn video {video_id} has no media files"),
            ));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("LoveHomePorn video {video_id} has no usable media"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("display_id", display_id);
        info.insert("age_limit", serde_json::json!(18));
        info.insert_if_some(
            "thumbnail",
            xml_element_text(&xml, "image")
                .or_else(|| xml_element_text(&xml, "thumb"))
                .map(|value| value.trim().to_owned()),
        );
        info.insert_if_some(
            "duration",
            xml_element_text(&xml, "duration").and_then(|value| value.trim().parse::<f64>().ok()),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

fn nuevo_html5_thumbnail(page_url: &str, html: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<(?:video|audio)\b[^>]*\bposter\s*=\s*["']([^"']+)["']"#,
    )
    .ok()?;
    let value = matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())?;
    Some(resolve_url(page_url, &value))
}

fn nuevo_html5_duration(html: &str) -> Option<f64> {
    let matcher = Regex::new(
        r#"(?is)<(?:video|audio)\b[^>]*\b(?:data-duration|duration)\s*=\s*["']?([0-9]+(?:\.[0-9]+)?)"#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<f64>().ok())
}

fn nuevo_xml_formats(xml: &str) -> Vec<serde_json::Value> {
    [("file", "sd"), ("filehd", "hd")]
        .into_iter()
        .filter_map(|(element, format_id)| {
            let media_url = xml_element_text(xml, element)?;
            let media_url = media_url.trim().to_owned();
            (!media_url.is_empty()).then(|| {
                let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
                serde_json::json!({
                    "url": media_url,
                    "format_id": format_id,
                    "ext": ext,
                })
            })
        })
        .collect()
}
