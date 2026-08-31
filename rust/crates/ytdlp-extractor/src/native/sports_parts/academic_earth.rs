/// Native Academic Earth course playlist extractor.
pub struct AcademicEarthCourseExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AcademicEarthCourseExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AcademicEarthCourseExtractor {
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
        let playlist_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Academic Earth playlist URL has no ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(
            r#"(?is)<h1\b[^>]*\bclass\s*=\s*["'][^"']*playlist-name[^"']*["'][^>]*>(.*?)</h1>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Academic Earth playlist {playlist_id} has no title"),
            )
        })?;
        let description =
            Regex::new(r#"(?is)<p\b[^>]*\bclass\s*=\s*["'][^"']*excerpt[^"']*["'][^>]*>(.*?)</p>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
        let link_matcher = Regex::new(
            r#"(?is)<li\b[^>]*\bclass\s*=\s*["'][^"']*lecture-preview[^"']*["'][^>]*>\s*<a\b[^>]*\btarget\s*=\s*["']_blank["'][^>]*\bhref\s*=\s*["']([^"']+)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Academic Earth lecture matcher: {error}"),
            )
        })?;
        let base_url = url::Url::parse(url).ok();
        let mut entries = Vec::new();
        for captures in link_matcher.captures_iter(&html).flatten() {
            let Some(raw_url) = captures.get(1).map(|value| value.as_str().trim()) else {
                continue;
            };
            let entry_url = base_url
                .as_ref()
                .and_then(|base| base.join(raw_url).ok())
                .map_or_else(
                    || proto_relative_url(raw_url, "https:"),
                    |value| value.to_string(),
                );
            entries.push(native_url_result(&entry_url));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
