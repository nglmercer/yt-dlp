/// Native CanalC2 archive-page extractor with explicit RTMP metadata.
pub struct Canalc2Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Canalc2Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Canalc2Extractor {
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
                    "CanalC2 URL has no video ID",
                )
            })?;
        let page_url = format!("http://www.canalc2.tv/video/{video_id}");
        let response = context.get(&page_url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let title = Regex::new(
            r#"(?is)\bclass\s*=\s*["'][^"']*col_description[^"']*["'][^>]*>.*?<h3\b[^>]*>(.*?)</h3>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| video_id.clone());
        let mut formats = Vec::new();
        let file_matcher =
            Regex::new(r#"(?is)\bfile\s*=\s*(["'])(.*?)\1"#).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid CanalC2 media matcher: {error}"),
                )
            })?;
        for captures in file_matcher.captures_iter(&webpage).flatten() {
            let Some(media_url) = captures.get(2).map(|value| value.as_str().trim()) else {
                continue;
            };
            if media_url.is_empty() {
                continue;
            }
            if media_url.starts_with("rtmp://") {
                let mut format = serde_json::Map::new();
                format.insert("url".to_owned(), serde_json::json!(media_url));
                format.insert("format_id".to_owned(), serde_json::json!("rtmp"));
                format.insert("ext".to_owned(), serde_json::json!("flv"));
                format.insert("protocol".to_owned(), serde_json::json!("rtmp"));
                if let Ok(rtmp_matcher) =
                    Regex::new(r#"^(rtmp://[^/]+/(?P<app>.+/))(?P<play_path>mp4:.+)$"#)
                {
                    if let Some(rtmp) = rtmp_matcher.captures(media_url).ok().flatten() {
                        if let Some(app) = rtmp.name("app") {
                            format.insert("app".to_owned(), serde_json::json!(app.as_str()));
                        }
                        if let Some(play_path) = rtmp.name("play_path") {
                            format.insert(
                                "play_path".to_owned(),
                                serde_json::json!(play_path.as_str()),
                            );
                        }
                    }
                }
                format.insert("page_url".to_owned(), serde_json::json!(url));
                formats.push(serde_json::Value::Object(format));
            } else {
                let resolved_url = resolve_url(&page_url, media_url);
                let extension = yt_dlp_core::determine_ext(Some(&resolved_url), "mp4");
                formats.push(serde_json::json!({
                    "url": resolved_url,
                    "format_id": "http",
                    "ext": extension,
                    "protocol": if extension == "m3u8" { "m3u8_native" } else { "http" },
                }));
            }
        }
        if formats.is_empty() {
            formats = html5_media_formats(&page_url, &webpage);
        }
        let first_format = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CanalC2 video {video_id} has no media formats"),
            )
        })?;
        let first_url = first_format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("CanalC2 video {video_id} has an invalid first format"),
                )
            })?;
        let first_ext = first_format
            .get("ext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("mp4");
        let duration = Regex::new(
            r#"(?is)\bid\s*=\s*["']video_duree["'][^>]*>([^<]+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .and_then(|value| yt_dlp_core::parse_duration(value.as_str().trim()));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!(first_ext));
        info.insert_if_some("duration", duration);
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
