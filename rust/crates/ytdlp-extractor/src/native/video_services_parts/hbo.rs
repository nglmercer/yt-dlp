/// Native HBO page-state and XML media extractor.
pub struct HboExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HboExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HboExtractor {
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
            .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "HBO URL has no ID"))?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let state = html_data_json_attribute(&webpage, "state").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HBO video {display_id} has no page state"),
            )
        })?;
        let location = state
            .get("video")
            .and_then(|video| json_string(video, "locationUrl"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("HBO video {display_id} has no XML location"),
                )
            })?;
        let xml_url = resolve_url(url, location);
        let xml_response = context.get(&xml_url)?;
        let xml = String::from_utf8_lossy(xml_response.body());
        let video_id = xml_element_text(&xml, "id").unwrap_or(display_id);
        let episode_title = xml_element_text(&xml, "title").unwrap_or_else(|| video_id.clone());
        let series = xml_element_text(&xml, "program");
        let title = series
            .as_ref()
            .map(|series| format!("{series} - {episode_title}"))
            .unwrap_or_else(|| episode_title.clone());
        let formats = hbo_formats(&xml, &video_id);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HBO video {video_id} has no playable sources"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let thumbnails = hbo_thumbnails(&xml);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("series", series);
        info.insert("episode", serde_json::json!(episode_title));
        info.insert_if_some(
            "duration",
            xml_element_text(&xml, "tv14").and_then(|value| yt_dlp_core::parse_duration(value.trim())),
        );
        info.insert("formats", serde_json::Value::Array(formats));
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
        info.insert("thumbnails", serde_json::Value::Array(thumbnails.clone()));
        info.insert_if_some(
            "thumbnail",
            thumbnails
                .first()
                .and_then(|thumbnail| thumbnail.get("url"))
                .cloned(),
        );
        if let Some(caption_url) = xml_element_text(&xml, "captionUrl")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            info.insert(
                "subtitles",
                serde_json::json!({"en": [{"url": caption_url, "ext": "ttml"}]}),
            );
        } else {
            info.insert("subtitles", serde_json::json!({}));
        }
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn hbo_formats(xml: &str, video_id: &str) -> Vec<serde_json::Value> {
    let sources_xml = hbo_element_inner(xml, "sources").unwrap_or_else(|| xml.to_owned());
    let xml = sources_xml.as_str();
    let mut formats = Vec::new();
    let Ok(size_matcher) = Regex::new(r#"(?is)<size\b([^>]*)>(.*?)</size\s*>"#) else {
        return formats;
    };
    for (index, captures) in size_matcher.captures_iter(xml).flatten().enumerate() {
        let Some(attributes) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(inner) = captures.get(2).map(|value| value.as_str()) else {
            continue;
        };
        let Some(media_url) = xml_element_text(inner, "path")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        else {
            continue;
        };
        let source_label = hbo_attribute(attributes, "width").unwrap_or_default();
        let (width, height) = hbo_size_info(&source_label);
        if let Some((base_url, app, play_path)) = hbo_rtmp_parts(&media_url) {
            let mut format = serde_json::json!({
                "url": base_url,
                "play_path": play_path,
                "app": app,
                "format_id": format!("rtmp-{index}"),
                "ext": "flv",
            });
            if let Some(width) = width {
                format["width"] = serde_json::json!(width);
            }
            if let Some(height) = height {
                format["height"] = serde_json::json!(height);
            }
            formats.push(format);
            continue;
        }
        let mut format = serde_json::json!({
            "url": media_url,
            "format_id": if height.is_some() {
                format!("http-{}p", height.unwrap_or_default())
            } else {
                "http".to_owned()
            },
            "ext": yt_dlp_core::determine_ext(Some(&media_url), "mp4"),
        });
        if let Some(width) = width {
            format["width"] = serde_json::json!(width);
        }
        if let Some(height) = height {
            format["height"] = serde_json::json!(height);
        }
        hbo_normalize_manifest(&mut format, video_id);
        formats.push(format);
    }

    for tag in ["tarball", "hls", "dash", "pro7", "1920", "pro6", "640", "pro5", "highwifi", "high3g", "medwifi", "med3g"] {
        let pattern = format!(r#"(?is)<{tag}\b[^>]*>(.*?)</{tag}\s*>"#);
        let Ok(matcher) = Regex::new(&pattern) else {
            continue;
        };
        for captures in matcher.captures_iter(xml).flatten() {
            let Some(raw_url) = captures.get(1).map(|value| hbo_xml_text(value.as_str())) else {
                continue;
            };
            if !(raw_url.starts_with("http://") || raw_url.starts_with("https://")) {
                continue;
            }
            let (media_url, protocol) = match tag {
                "tarball" => (raw_url.replace(".tar", "/base_index_w8.m3u8"), "m3u8_native"),
                "hls" => (raw_url.replace(".tar", "/base_index.m3u8"), "m3u8_native"),
                "dash" => (raw_url.replace(".tar", "/manifest.mpd"), "http_dash_segments"),
                _ => (raw_url, "http"),
            };
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": tag,
                "protocol": protocol,
                "ext": if protocol == "http" { "mp4" } else { "mp4" },
            });
            hbo_normalize_manifest(&mut format, video_id);
            formats.push(format);
        }
    }
    formats
}

fn hbo_thumbnails(xml: &str) -> Vec<serde_json::Value> {
    let thumbnails_xml = hbo_element_inner(xml, "titleCardSizes").unwrap_or_default();
    let Ok(matcher) = Regex::new(r#"(?is)<size\b([^>]*)>(.*?)</size\s*>"#) else {
        return Vec::new();
    };
    matcher
        .captures_iter(&thumbnails_xml)
        .flatten()
        .filter_map(|captures| {
            let attributes = captures.get(1).map(|value| value.as_str())?;
            let inner = captures.get(2).map(|value| value.as_str())?;
            let url = xml_element_text(inner, "path")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
            let mut thumbnail = serde_json::json!({"url": url});
            if let Some(width) = hbo_attribute(attributes, "width").and_then(|value| value.parse::<i64>().ok()) {
                thumbnail["width"] = serde_json::json!(width);
                thumbnail["id"] = serde_json::json!(width);
            }
            Some(thumbnail)
        })
        .collect()
}

fn hbo_size_info(width: &str) -> (Option<i64>, Option<i64>) {
    match width {
        "pro7" | "1920" => (Some(1280), Some(720)),
        "pro6" | "640" => (Some(768), Some(432)),
        "pro5" | "highwifi" | "high3g" => (Some(640), Some(360)),
        "medwifi" | "med3g" => (Some(400), Some(224)),
        _ => (None, None),
    }
}

fn hbo_normalize_manifest(format: &mut serde_json::Value, _video_id: &str) {
    if format.get("ext").and_then(serde_json::Value::as_str) == Some("m3u8") {
        format["ext"] = serde_json::json!("mp4");
        format["protocol"] = serde_json::json!("m3u8_native");
    }
    if format.get("ext").and_then(serde_json::Value::as_str) == Some("mpd") {
        format["ext"] = serde_json::json!("mp4");
        format["protocol"] = serde_json::json!("http_dash_segments");
    }
}

fn hbo_rtmp_parts(url: &str) -> Option<(String, String, String)> {
    let matcher = Regex::new(r#"^(rtmpe?://[^/]+/(?P<app>.+))/((?P<playpath>mp4:.+))$"#).ok()?;
    let captures = matcher.captures(url).ok().flatten()?;
    Some((
        captures.get(1)?.as_str().to_owned(),
        captures.name("app")?.as_str().to_owned(),
        captures.name("playpath")?.as_str().to_owned(),
    ))
}

fn hbo_xml_text(value: &str) -> String {
    html_text_fragment(value)
        .trim()
        .trim_start_matches("<![CDATA[")
        .trim_end_matches("]]>")
        .trim()
        .to_owned()
}

fn hbo_element_inner(xml: &str, element: &str) -> Option<String> {
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

fn hbo_attribute(html: &str, name: &str) -> Option<String> {
    let name = regex::escape(name);
    for pattern in [
        format!(r#"(?is)(?:^|\s){name}\s*=\s*"([^"]*)""#),
        format!(r#"(?is)(?:^|\s){name}\s*=\s*'([^']*)'"#),
    ] {
        if let Some(value) = Regex::new(&pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
        {
            return Some(value);
        }
    }
    None
}
