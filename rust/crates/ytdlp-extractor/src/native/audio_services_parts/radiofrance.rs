/// Native Radio France Radiovisions audio-source-map extractor.
pub struct RadioFranceExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RadioFranceExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RadioFranceExtractor {
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
                    "Radio France Radiovisions URL has no ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let title = radiofrance_title(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Radio France Radiovisions {video_id} has no title"),
            )
        })?;
        let source_map = radiofrance_source_map(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Radio France Radiovisions {video_id} has no audio URLs"),
            )
        })?;
        let formats = radiofrance_formats(&source_map);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Radio France Radiovisions {video_id} has no usable audio URLs"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some(
            "description",
            radiofrance_block_text(&webpage, "bloc_page_wrapper", "text"),
        );
        info.insert_if_some("uploader", radiofrance_uploader(&webpage));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn radiofrance_title(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<h1\b[^>]*>(.*?)</h1\s*>"#).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn radiofrance_source_map(html: &str) -> Option<String> {
    [
        r#"(?is)<[^>]*\bclass\s*=\s*"[^"]*\bjp-jplayer\b[^"]*"[^>]*\bdata-source\s*=\s*"([^"]+)""#,
        r#"(?is)<[^>]*\bclass\s*=\s*'[^']*\bjp-jplayer\b[^']*'[^>]*\bdata-source\s*=\s*'([^']+)'"#,
    ]
    .iter()
    .find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
    })
}

fn radiofrance_formats(source_map: &str) -> Vec<serde_json::Value> {
    let Ok(matcher) = Regex::new(r#"(?i)([a-z0-9]+)\s*:\s*'([^']+)'"#) else {
        return Vec::new();
    };
    matcher
        .captures_iter(source_map)
        .flatten()
        .enumerate()
        .filter_map(|(quality, captures)| {
            let format_id = captures.get(1)?.as_str().to_owned();
            let media_url = unescape_html_attribute(captures.get(2)?.as_str());
            if !(media_url.starts_with("http://") || media_url.starts_with("https://")) {
                return None;
            }
            let extension = yt_dlp_core::determine_ext(Some(&media_url), &format_id);
            Some(serde_json::json!({
                "format_id": format_id,
                "url": media_url,
                "ext": extension,
                "vcodec": "none",
                "quality": quality as i64,
            }))
        })
        .collect()
}

fn radiofrance_block_text(html: &str, outer_class: &str, inner_class: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<div\b[^>]*\bclass\s*=\s*["']{}["'][^>]*>\s*<div\b[^>]*\bclass\s*=\s*["']{}["'][^>]*>(.*?)</div\s*>"#,
        regex::escape(outer_class),
        regex::escape(inner_class),
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn radiofrance_uploader(html: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<div\b[^>]*\bclass\s*=\s*["']credit["'][^>]*>&nbsp;&nbsp;&copy;&nbsp;(.*?)</div\s*>"#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}
