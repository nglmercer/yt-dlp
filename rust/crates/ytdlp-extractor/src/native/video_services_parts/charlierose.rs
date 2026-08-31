/// Native Charlie Rose HTML5 player extractor.
pub struct CharlieRoseExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CharlieRoseExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CharlieRoseExtractor {
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
                    "Charlie Rose URL has no video ID",
                )
            })?;
        let player_url = format!("https://charlierose.com/video/player/{video_id}");
        let response = context.get(&player_url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let formats = html5_media_formats(&player_url, &webpage);
        let first_format = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Charlie Rose video {video_id} has no HTML5 media sources"),
            )
        })?;
        let title = html_meta_value(&webpage, "og:title")
            .map(|value| html_text_fragment(&value))
            .map(|value| {
                value
                    .strip_suffix(" - Charlie Rose")
                    .unwrap_or(&value)
                    .trim()
                    .to_owned()
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| video_id.clone());
        let first_url = first_format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Charlie Rose video {video_id} has an invalid first source"),
                )
            })?;
        let first_ext = first_format
            .get("ext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("mp4");
        let subtitles = charlierose_subtitles(&player_url, &webpage);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!(first_ext));
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        info.insert_if_some("description", html_meta_value(&webpage, "og:description"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("subtitles", subtitles);
        Ok(ExtractorResult::single(info))
    }
}

fn charlierose_subtitles(page_url: &str, html: &str) -> Option<serde_json::Value> {
    let matcher = Regex::new(r#"(?is)<track\b[^>]*\bsrc\s*=\s*["']([^"']+)["'][^>]*>"#).ok()?;
    let language_matcher =
        Regex::new(r#"(?is)\b(?:srclang|language|lang)\s*=\s*["']([^"']+)["']"#).ok()?;
    let mut subtitles = serde_json::Map::new();
    for captures in matcher.captures_iter(html).flatten() {
        let Some(raw_url) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let subtitle_url = resolve_url(page_url, &unescape_html_attribute(raw_url));
        let track = captures.get(0).map(|value| value.as_str()).unwrap_or_default();
        let language = language_matcher
            .captures(track)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| "en".to_owned());
        let entries = subtitles
            .entry(language)
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let serde_json::Value::Array(entries) = entries {
            entries.push(serde_json::json!({
                "url": subtitle_url,
                "ext": yt_dlp_core::determine_ext(Some(raw_url), "vtt"),
            }));
        }
    }
    (!subtitles.is_empty()).then_some(serde_json::Value::Object(subtitles))
}
