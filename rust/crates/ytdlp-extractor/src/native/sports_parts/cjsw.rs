/// Native CJSW episode audio-page extractor.
pub struct CjswExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CjswExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CjswExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "CJSW URL did not match its native pattern",
            )
        })?;
        let program = captures
            .name("program")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CJSW URL has no program")
            })?;
        let episode_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CJSW URL has no episode ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(
            r#"(?is)<h1\b[^>]*\bclass\s*=\s*["'][^"']*episode-header__title[^"']*["'][^>]*>([^<]+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str().trim()))
        .or_else(|| {
            Regex::new(r#"(?is)\bdata-audio-title\s*=\s*["']([^"']+)["']"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| unescape_html_attribute(value.as_str().trim()))
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CJSW episode {episode_id} has no title"),
            )
        })?;
        let audio_url = Regex::new(r#"(?is)<button\b[^>]*\bdata-audio-src\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("CJSW episode {episode_id} has no audio URL"),
                )
            })?;
        let audio_id =
            Regex::new(r#"(?i)/([\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12})\.mp3"#)
                .ok()
                .and_then(|matcher| matcher.captures(&audio_url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
                .unwrap_or_else(|| format!("{program}/{episode_id}"));
        let ext = yt_dlp_core::determine_ext(Some(&audio_url), "mp3");
        let description = Regex::new(r#"(?is)<p\b[^>]*>(.*?)</p>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let series = Regex::new(r#"(?is)\bdata-showname\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .unwrap_or(program);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert("series", serde_json::json!(series));
        info.insert("episode_id", serde_json::json!(episode_id));
        info.insert("url", serde_json::json!(audio_url.clone()));
        info.insert("ext", serde_json::json!(ext));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": audio_url,
                "format_id": "source",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
