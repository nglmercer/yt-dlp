/// Native Lecture2Go page/player extractor.
pub struct Lecture2GoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Lecture2GoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Lecture2GoExtractor {
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
                    "Lecture2Go URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let title = Regex::new(r#"(?is)<em[^>]+class\s*=\s*["']title["'][^>]*>(.+?)</em>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Lecture2Go video {video_id} has no title"),
                )
            })?;
        let formats = lecture2go_formats(&webpage, &video_id)?;
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Lecture2Go video {video_id} has no playable sources"),
            ));
        }
        let creator = Regex::new(
            r#"(?is)<div[^>]+id\s*=\s*["']description["'][^>]*>([^<]+)</div>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty());
        let duration = Regex::new(
            r#"(?is)Duration:\s*</em>\s*<em[^>]*>([^<]+)</em>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .and_then(|value| yt_dlp_core::parse_duration(value.as_str().trim()));
        let view_count = Regex::new(
            r#"(?is)Views:\s*</em>\s*<em[^>]+>(\d+)</em>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i64>().ok());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("creator", creator);
        info.insert_if_some("duration", duration);
        info.insert_if_some("view_count", view_count);
        Ok(ExtractorResult::single(info))
    }
}
