fn winsports_drupal_settings(html: &str) -> Option<serde_json::Value> {
    let matcher = Regex::new(
        r#"(?is)<script\b[^>]*\bdata-drupal-selector\s*=\s*["']drupal-settings-json["'][^>]*>(.*?)</script\s*>"#,
    )
    .ok()?;
    let body = matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))?
        .as_str();
    serde_json::from_str(body.trim()).ok()
}

fn winsports_nested_url(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(values) => {
            if let Some(url) = json_string(value, "url").filter(|value| mediastream_embed_url(value))
            {
                return Some(url.to_owned());
            }
            values.values().find_map(winsports_nested_url)
        }
        serde_json::Value::Array(values) => values.iter().find_map(winsports_nested_url),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => None,
    }
}

fn winsports_title(html: &str) -> Option<String> {
    let title = html_json_ld(html)
        .and_then(|json_ld| json_string(&json_ld, "name").map(str::to_owned))
        .or_else(|| html_meta_value(html, "og:title"))
        .map(|value| html_text_fragment(&value))?;
    let title = title
        .strip_suffix("| Win Sports")
        .map_or(title.as_str(), str::trim)
        .trim();
    (!title.is_empty()).then(|| title.to_owned())
}

/// Native WinSports page-to-MediaStream transparent redirect.
pub struct WinSportsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WinSportsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WinSportsExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "WinSports URL has no display ID",
                )
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let media_url = winsports_drupal_settings(&html)
            .and_then(|settings| winsports_nested_url(&settings))
            .or_else(|| mediastream_find_embed_url(&html))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("WinSports page {display_id} has no MediaStream embed"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("_type", serde_json::json!("url_transparent"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ie_key", serde_json::json!("MediaStream"));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", winsports_title(&html));
        Ok(ExtractorResult::single(info))
    }
}
