/// Native DBTV transparent wrapper for YouTube and JWPlatform entries.
pub struct DbtvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DbtvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DbtvExtractor {
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
        _context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "DBTV URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DBTV URL has no video ID")
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned());
        let (target_url, ie_key) = if video_id.len() == 11 {
            (video_id.clone(), "Youtube")
        } else {
            (format!("jwplatform:{video_id}"), "JWPlatform")
        };
        let mut info = InfoDict::new();
        info.insert("_type", serde_json::json!("url_transparent"));
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("display_id", display_id);
        info.insert("url", serde_json::json!(target_url));
        info.insert("ie_key", serde_json::json!(ie_key));
        Ok(ExtractorResult::single(info))
    }
}
