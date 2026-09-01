/// Native Heise article/video extractor.
///
/// VideoOut XML is implemented directly. Kaltura and YouTube embeds remain
/// explicit TODOs until their media backends are native Rust implementations.
pub struct HeiseExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HeiseExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HeiseExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Heise URL has no video ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let title = heise_title(&webpage, &video_id);
        let description = html_meta_value(&webpage, "og:description")
            .or_else(|| html_meta_value(&webpage, "description"));

        if heise_contains_kaltura(&webpage) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Heise video {video_id} requires a native Kaltura extractor"
                ),
            ));
        }
        if heise_contains_youtube(&webpage) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Heise video {video_id} requires a native YouTube extractor"
                ),
            ));
        }

        let mut feed = heise_feed_parameters(&webpage);
        if feed.0.is_none() || feed.1.is_none() {
            feed.0 = heise_attribute_by_class(&webpage, "videoplayerjw", "data-container");
            feed.1 = heise_attribute_by_class(&webpage, "videoplayerjw", "data-sequenz");
        }
        let (container, sequence) = match (feed.0, feed.1) {
            (Some(container), Some(sequence)) => (container, sequence),
            _ => {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Heise video {video_id} has no VideoOut feed parameters"),
                ))
            }
        };
        let mut request = Request::new("http://www.heise.de/videout/feed");
        request.update_query(&[
            ("container".to_owned(), container),
            ("sequenz".to_owned(), sequence),
        ]);
        let feed_response = context.request(&request)?;
        let xml = String::from_utf8_lossy(feed_response.body());
        let formats = heise_feed_formats(&xml);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Heise video {video_id} has no playable VideoOut sources"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", xml_element_text(&xml, "image"));
        info.insert_if_some(
            "timestamp",
            html_meta_value(&webpage, "date").and_then(parse_timestamp),
        );
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
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn heise_title(html: &str, video_id: &str) -> String {
    let meta_title = html_meta_value(html, "fulltitle")
        .or_else(|| html_meta_value(html, "title"))
        .filter(|value| value != "c't");
    if let Some(title) = meta_title {
        return title;
    }
    if let Some(title) = heise_capture(
        html,
        r#"(?is)<div\b[^>]*\bclass\s*=\s*["'][^"']*\bvideoplayerjw\b[^"']*["'][^>]*\bdata-title\s*=\s*["']([^"']+)"#,
    ) {
        return unescape_html_attribute(&title);
    }
    heise_capture(
        html,
        r#"(?is)<h1\b[^>]*\bclass\s*=\s*["'][^"']*\barticle_page_title\b[^"']*["'][^>]*>(.*?)<"#,
    )
    .map(|value| html_text_fragment(&value))
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| video_id.to_owned())
}

fn heise_feed_parameters(html: &str) -> (Option<String>, Option<String>) {
    let query = heise_capture(html, r#"(?is)/videout/feed\.json\?([^']+)"#);
    (
        query
            .as_deref()
            .and_then(|value| heise_query_value(value, "container")),
        query
            .as_deref()
            .and_then(|value| heise_query_value(value, "sequenz")),
    )
}

fn heise_query_value(query: &str, key: &str) -> Option<String> {
    let pattern = format!(r#"(?i)(?:^|&){}\s*=\s*([^&"'\\s]+)"#, regex::escape(key));
    heise_capture(query, &pattern)
}

fn heise_feed_formats(xml: &str) -> Vec<serde_json::Value> {
    let Ok(source_matcher) = Regex::new(r#"(?is)<source\b[^>]*>"#) else {
        return Vec::new();
    };
    let mut formats = Vec::new();
    for (index, source_tag) in source_matcher.find_iter(xml).flatten().enumerate() {
        let source_tag = source_tag.as_str();
        let Some(media_url) = heise_attribute(source_tag, "file")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        else {
            continue;
        };
        let label = heise_attribute(source_tag, "label").unwrap_or_else(|| format!("source-{index}"));
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let mut format = serde_json::json!({
            "url": media_url,
            "format_note": label,
            "format_id": format!("{extension}_{label}"),
            "ext": extension,
        });
        if let Some(height) = heise_capture(&label, r#"(?i)(\d+)p$"#)
            .and_then(|value| value.parse::<i64>().ok())
        {
            format["height"] = serde_json::json!(height);
        }
        if extension == "m3u8" {
            format["ext"] = serde_json::json!("mp4");
            format["protocol"] = serde_json::json!("m3u8_native");
        }
        formats.push(format);
    }
    formats
}

fn heise_contains_kaltura(html: &str) -> bool {
    html.to_ascii_lowercase().contains("kaltura") || html.contains("entry-id=")
}

fn heise_contains_youtube(html: &str) -> bool {
    Regex::new(r#"(?i)(?:youtube\.com|youtu\.be)/"#)
        .ok()
        .and_then(|matcher| matcher.find(html).ok().flatten())
        .is_some()
}

fn heise_attribute_by_class(html: &str, class: &str, attribute: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<[a-z0-9]+\b[^>]*>"#).ok()?;
    matcher.find_iter(html).flatten().find_map(|tag| {
        let tag = tag.as_str();
        let classes = heise_attribute(tag, "class")?;
        classes
            .split_ascii_whitespace()
            .any(|value| value == class)
            .then(|| heise_attribute(tag, attribute))?
    })
}

fn heise_attribute(html: &str, name: &str) -> Option<String> {
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

fn heise_capture(html: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}
