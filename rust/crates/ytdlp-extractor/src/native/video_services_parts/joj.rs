/// Native JOJ embed bitrate/XML extractor.
pub struct JojExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JojExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JojExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "JOJ URL has no video ID")
            })?;
        let embed_url = format!("https://media.joj.sk/embed/{video_id}");
        let response = context.get(&embed_url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let title = joj_video_title(&webpage)
            .or_else(|| html_title_value(&webpage))
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .unwrap_or_else(|| video_id.clone());
        let bitrates = json_object_after_marker(&webpage, "bitrates")
            .or_else(|| json_object_after_marker(&webpage, "src ="));
        let mut formats = bitrates
            .as_ref()
            .map(|bitrates| joj_bitrate_formats(bitrates))
            .unwrap_or_default();
        if formats.is_empty() {
            let xml_url = format!("https://media.joj.sk/services/Video.php?clip={video_id}");
            let xml_response = context.get(&xml_url)?;
            let xml = String::from_utf8_lossy(xml_response.body());
            formats = joj_xml_formats(&xml);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JOJ video {video_id} has no playable MP4 sources"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        info.insert_if_some("duration", joj_duration(&webpage));
        info.insert(
            "url",
            first
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
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

fn joj_video_title(webpage: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)\bvideoTitle\s*:\s*["']([^"']+)["']"#).ok()?;
    matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
}

fn joj_duration(webpage: &str) -> Option<i64> {
    let matcher = Regex::new(r#"(?is)\bvideoDuration\s*:\s*(\d+)"#).ok()?;
    matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse().ok())
}

fn joj_bitrate_formats(bitrates: &serde_json::Value) -> Vec<serde_json::Value> {
    bitrates
        .get("mp4")
        .into_iter()
        .flat_map(json_object_values)
        .filter_map(|value| value.as_str())
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(|media_url| {
            let height = joj_height(media_url);
            let mut format = serde_json::json!({
                "url": media_url,
                "ext": "mp4",
                "protocol": "http",
            });
            if let Some(height) = height {
                format["format_id"] = serde_json::json!(format!("{height}p"));
                format["height"] = serde_json::json!(height);
            } else {
                format["format_id"] = serde_json::json!("source");
            }
            format
        })
        .collect()
}

fn joj_xml_formats(xml: &str) -> Vec<serde_json::Value> {
    let Some(matcher) = Regex::new(r#"(?is)<file\b([^>]*)>"#).ok() else {
        return Vec::new();
    };
    let mut formats = Vec::new();
    for captures in matcher.captures_iter(xml).flatten() {
        let Some(attributes) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(path) = joj_attribute(attributes, "path") else {
            continue;
        };
        let path = path.strip_prefix("dat/").unwrap_or(&path);
        let media_url = format!("http://n16.joj.sk/storage/{path}");
        let format_id = joj_attribute(attributes, "id")
            .or_else(|| joj_attribute(attributes, "label"))
            .unwrap_or_else(|| "source".to_owned());
        let mut format = serde_json::json!({
            "url": media_url,
            "format_id": format_id,
            "ext": "mp4",
            "protocol": "http",
        });
        if let Some(height) = joj_height(&format_id) {
            format["height"] = serde_json::json!(height);
        }
        formats.push(format);
    }
    formats
}

fn joj_attribute(attributes: &str, name: &str) -> Option<String> {
    let matcher = Regex::new(&format!(
        r#"(?is)\b{}\s*=\s*["']([^"']+)["']"#,
        regex::escape(name)
    ))
    .ok()?;
    matcher
        .captures(attributes)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
}

fn joj_height(value: &str) -> Option<i64> {
    let matcher = Regex::new(r#"(?i)(\d+)[pP]"#).ok()?;
    if let Some(height) = matcher
        .captures(value)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse().ok())
    {
        return Some(height);
    }
    value.to_ascii_lowercase().contains("pal.").then_some(576)
}
