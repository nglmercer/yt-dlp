/// Native FAZ.net embedded XML encoding extractor.
pub struct FazExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FazExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FazExtractor {
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
        let video_id = faz_match_id(&self.matcher, url)?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let media = Regex::new(r#"(?is)\bdata-videojs-media\s*=\s*'([^']+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
            .map(|value| unescape_html_attribute(&value))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FAZ page {video_id} has no videojs media XML"),
                )
            })?;
        if media.trim().eq_ignore_ascii_case("extern") {
            let perform_url = Regex::new(
                r#"(?is)<iframe\b[^>]*\bsrc\s*=\s*['"]((?:https?:)?//player\.performgroup\.com/eplayer/eplayer\.html#/??[0-9a-f]{26}\.[0-9a-z]{26})"#,
            )
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str()))
            .map(|value| proto_relative_url(value, "http:"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FAZ page {video_id} has no Perform player URL"),
                )
            })?;
            return Ok(ExtractorResult::Redirect {
                url: perform_url,
                ie_key: None,
            });
        }

        let mut formats = Vec::new();
        for (quality, code) in ["LOW", "HIGH", "HQ"].iter().enumerate() {
            let Some(encoding) = faz_xml_block(&media, code) else {
                continue;
            };
            let Some(media_url) = faz_xml_field(&encoding, "FILENAME") else {
                continue;
            };
            let media_url = unescape_html_attribute(&media_url);
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": code.to_ascii_lowercase(),
                "quality": quality,
                "protocol": "http",
                "ext": "mp4",
            });
            if let Some(vcodec) = faz_xml_field(&encoding, "CODEC") {
                format["vcodec"] = serde_json::json!(vcodec);
            }
            let xml_tbr = faz_xml_field(&encoding, "AVERAGEBITRATE")
                .and_then(|value| value.replace(',', ".").parse::<f64>().ok());
            if let Some(captures) = Regex::new(r#"(?i)(\d+)x(\d+)_(\d+)\.mp4"#)
                .ok()
                .and_then(|matcher| matcher.captures(media_url.as_str()).ok().flatten())
            {
                if let Some(width) = captures.get(1).and_then(|value| value.as_str().parse::<i64>().ok()) {
                    format["width"] = serde_json::json!(width);
                }
                if let Some(height) = captures.get(2).and_then(|value| value.as_str().parse::<i64>().ok()) {
                    format["height"] = serde_json::json!(height);
                }
                if let Some(tbr) = xml_tbr.or_else(|| captures.get(3).and_then(|value| value.as_str().parse::<f64>().ok())) {
                    format["tbr"] = serde_json::json!(tbr);
                }
            } else if let Some(tbr) = xml_tbr {
                format["tbr"] = serde_json::json!(tbr);
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FAZ video {video_id} has no playable encodings"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", html_meta_value(&webpage, "og:title"));
        info.insert_if_some(
            "description",
            html_meta_value(&webpage, "og:description")
                .map(|value| unescape_html_attribute(&value).trim().to_owned()),
        );
        info.insert_if_some("thumbnail", xml_element_text(&media, "STILL_BIG"));
        info.insert_if_some(
            "duration",
            xml_element_text(&media, "DURATION").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert(
            "url",
            first_format.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

fn faz_xml_block(xml: &str, element: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<{}\b[^>]*>(.*?)</{}\s*>"#,
        regex::escape(element),
        regex::escape(element)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(xml).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn faz_xml_field(xml: &str, element: &str) -> Option<String> {
    faz_xml_block(xml, element).map(|value| html_text_fragment(&value))
}

fn faz_match_id(matcher: &Regex, url: &str) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "FAZ URL has no video ID")
        })
}
