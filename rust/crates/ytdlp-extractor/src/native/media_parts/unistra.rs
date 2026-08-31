/// Native UTV Strasbourg progressive-video extractor.
pub struct UnistraExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl UnistraExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for UnistraExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Unistra URL has no video ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body()).into_owned();
        let file_matcher = Regex::new(r#"(?is)\bfile\s*:\s*"([^"]+)""#).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Unistra media matcher: {error}"),
            )
        })?;
        let mut file_paths = Vec::new();
        for captures in file_matcher.captures_iter(&webpage).flatten() {
            let Some(file_path) = captures
                .get(1)
                .map(|value| unescape_html_attribute(value.as_str()))
            else {
                continue;
            };
            if !file_paths.contains(&file_path) {
                file_paths.push(file_path);
            }
        }
        if file_paths.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Unistra video {video_id} has no media files"),
            ));
        }

        let formats = file_paths
            .into_iter()
            .map(|file_path| {
                let format_id = if file_path.ends_with("-HD.mp4") {
                    "HD"
                } else {
                    "SD"
                };
                let media_url = format!("http://vod-flash.u-strasbg.fr:8080{file_path}");
                serde_json::json!({
                    "url": media_url,
                    "format_id": format_id,
                    "quality": if format_id == "HD" { 1 } else { 0 },
                    "ext": "mp4",
                })
            })
            .collect::<Vec<_>>();
        let first_format = formats.first().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Unistra video {video_id} has no usable media formats"),
            )
        })?;
        let title = Regex::new(r#"(?is)<title\b[^>]*>\s*UTV\s*-\s*(.*?)</title\b"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Unistra video {video_id} has no title"),
                )
            })?;
        let description = html_meta_value(&webpage, "Description")
            .map(|value| unescape_html_attribute(&value))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Unistra video {video_id} has no description"),
                )
            })?;
        let thumbnail = Regex::new(r#"(?is)\bimage\s*:\s*"([^"]+)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Unistra video {video_id} has no thumbnail"),
                )
            })?;
        let first_url = first_format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("description", serde_json::json!(description));
        info.insert("thumbnail", serde_json::json!(thumbnail));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
