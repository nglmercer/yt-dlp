/// Native CRTVG page metadata and fixed HLS/DASH manifest extractor.
pub struct CrtvgExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CrtvgExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CrtvgExtractor {
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
                    "CRTVG URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let video_url = Regex::new(r#"(?is)\bvar\s+url\s*=\s*["']([^"']+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("CRTVG video {video_id} has no stream URL"),
                )
            })?;
        let hls_url = format!("{video_url}/playlist.m3u8");
        let dash_url = format!("{video_url}/manifest.mpd");
        let formats = serde_json::json!([
            {
                "url": hls_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            },
            {
                "url": dash_url,
                "format_id": "dash",
                "protocol": "http_dash_segments",
                "ext": "mp4",
            }
        ]);
        let title = html_meta_value(&webpage, "og:title")
            .or_else(|| html_meta_value(&webpage, "twitter:title"))
            .map(|value| {
                value
                    .strip_suffix(" | CRTVG")
                    .unwrap_or(&value)
                    .to_owned()
            })
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("CRTVG video {video_id} has no title"),
                )
            })?;
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(video_id));
        output.insert("title", serde_json::json!(title));
        output.insert_if_some("description", html_meta_value(&webpage, "description"));
        output.insert_if_some(
            "thumbnail",
            html_meta_value(&webpage, "og:image")
                .or_else(|| html_meta_value(&webpage, "twitter:image")),
        );
        if let Some(old_id) = crtvg_old_archive_id(output.get_str("id").unwrap_or_default()) {
            output.insert("_old_archive_ids", serde_json::json!([old_id]));
        }
        output.insert("url", serde_json::json!(hls_url));
        output.insert("ext", serde_json::json!("mp4"));
        output.insert("formats", formats);
        output.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(output))
    }
}

fn crtvg_old_archive_id(video_id: &str) -> Option<String> {
    let (prefix, old_id) = video_id.rsplit_once('-')?;
    (!prefix.is_empty() && old_id.len() == 7 && old_id.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| format!("crtvg {old_id}"))
}
