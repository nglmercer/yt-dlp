/// Native HellPorno HTML5 video extractor.
pub struct HellPornoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HellPornoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HellPornoExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "HellPorno URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let formats = html5_media_formats(url, &webpage);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HellPorno video {display_id} has no HTML5 media formats"),
            ));
        }
        let video_id = hellporno_capture(
            &webpage,
            r#"(?is)chs_object\s*=\s*["'](\d+)"#,
        )
        .or_else(|| {
            hellporno_capture(
                &webpage,
                r#"(?is)params\s*\[\s*["']video_id["']\s*\]\s*=\s*(\d+)"#,
            )
        })
        .unwrap_or(display_id.clone());
        let raw_title = html_title_value(&webpage).unwrap_or_else(|| display_id.clone());
        let title = raw_title
            .strip_suffix(" - Hell Porno")
            .unwrap_or(&raw_title)
            .trim()
            .to_owned();
        let description = html_element_by_class(&webpage, "desc_video_view_v2")
            .map(|value| html_text_fragment(&value))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let categories = html_meta_value(&webpage, "keywords")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .map(serde_json::Value::String)
            .collect::<Vec<_>>();
        let duration = html_meta_value(&webpage, "video:duration")
            .and_then(|value| yt_dlp_core::parse_duration(&value));
        let release_date = html_meta_value(&webpage, "video:release_date");
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("description", description);
        info.insert("categories", serde_json::Value::Array(categories));
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        info.insert_if_some("duration", duration);
        info.insert_if_some(
            "timestamp",
            release_date.clone().and_then(parse_timestamp),
        );
        info.insert_if_some(
            "upload_date",
            release_date.as_deref().and_then(date_digits),
        );
        info.insert_if_some("view_count", hellporno_view_count(&webpage));
        info.insert("age_limit", serde_json::json!(18));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn hellporno_capture(html: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn hellporno_view_count(html: &str) -> Option<i64> {
    hellporno_capture(html, r#"(?is)>\s*Views\s+(\d+)"#)
        .and_then(|value| value.parse::<i64>().ok())
}
