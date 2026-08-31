/// Native Soundgasm single-audio extractor. Audio URLs and metadata are
/// embedded in the page's jPlayer markup and require no JavaScript execution.
pub struct SoundgasmExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SoundgasmExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SoundgasmExtractor {
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
                "Soundgasm URL did not match its native pattern",
            )
        })?;
        let user = captures
            .name("user")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Soundgasm URL has no user")
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Soundgasm URL has no title")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let audio_url = Regex::new(r#"\bm4a\s*:\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Soundgasm audio {display_id} has no m4a URL"),
                )
            })?;
        let title = Regex::new(
            r#"(?is)<div[^>]*\bclass\s*=\s*["'][^"']*\bjp-title\b[^"']*["'][^>]*>(.*?)</div>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| display_id.clone());
        let description = Regex::new(
            r#"(?is)<div[^>]*\bclass\s*=\s*["'][^"']*\bjp-description\b[^"']*["'][^>]*>(.*?)</div>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .or_else(|| {
            Regex::new(r#"(?is)<li>\s*Description:\s*(.*?)</li>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty())
        });
        let audio_id = Regex::new(r#"/([^/]+)\.m4a(?:[?#]|$)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&audio_url).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| display_id.clone());
        let extension = yt_dlp_core::determine_ext(Some(&audio_url), "m4a");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("url", serde_json::json!(audio_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": audio_url,
                "format_id": "audio",
                "ext": extension,
                "vcodec": "none",
            }]),
        );
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert("uploader", serde_json::json!(user));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Soundgasm profile playlist extractor. Profile pages expose links
/// to the same native audio pages, which are expanded in Rust for consistent
/// playlist selection and JSON output.
pub struct SoundgasmProfileExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SoundgasmProfileExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SoundgasmProfileExtractor {
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
                "Soundgasm profile URL did not match its native pattern",
            )
        })?;
        let profile_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Soundgasm profile has no ID",
                )
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let link_matcher =
            Regex::new(r#"(?is)\bhref\s*=\s*["']([^"']+)["']"#).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Soundgasm profile link matcher: {error}"),
                )
            })?;
        let base = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Soundgasm profile URL: {error}"),
            )
        })?;
        let audio_extractor = SoundgasmExtractor::new(ExtractorDescriptor::new(
            "SoundgasmIE",
            "soundgasm",
            r"https?://(?:www\.)?soundgasm\.net/u/(?P<user>[0-9a-zA-Z_-]+)/(?P<display_id>[0-9a-zA-Z_-]+)",
            true,
        ))?;
        let mut entries = Vec::new();
        let mut seen_links = Vec::new();
        for captures in link_matcher.captures_iter(&html).flatten() {
            let Some(raw_link) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let Some(link) = base.join(raw_link).ok().map(|value| value.to_string()) else {
                continue;
            };
            if !link.contains(&format!("/u/{profile_id}/")) || seen_links.contains(&link) {
                continue;
            }
            seen_links.push(link.clone());
            let entry = audio_extractor
                .extract_with_context(&link, context)
                .map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Soundgasm profile entry {link}: {error}"),
                    )
                })?;
            match entry {
                ExtractorResult::Single(info) => entries.push(info),
                ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                    return Err(ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Soundgasm profile entry {link} returned a non-audio result"),
                    ));
                }
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(profile_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
