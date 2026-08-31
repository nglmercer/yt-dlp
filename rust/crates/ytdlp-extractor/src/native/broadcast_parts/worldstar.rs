/// Native WorldStarHipHop HTML5 media extractor.
pub struct WorldStarHipHopExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WorldStarHipHopExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WorldStarHipHopExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "WorldStar URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(r#"(?is)<div\b[^>]*class\s*=\s*["'][^"']*content-heading[^"']*["'][^>]*>\s*<h1\b[^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                Regex::new(r#"(?is)<span\b[^>]*class\s*=\s*["'][^"']*tc-sp-pinned-title[^"']*["'][^>]*>(.*?)</span>"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&html).ok().flatten())
                    .and_then(|captures| captures.get(1))
                    .map(|value| html_text_fragment(value.as_str()))
                    .filter(|value| !value.trim().is_empty())
            })
            .or_else(|| html_meta_value(&html, "og:title"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("WorldStar video {video_id} has no title"),
                )
            })?;
        let formats = html5_media_formats(url, &html);
        if formats.is_empty() {
            let generic =
                GenericExtractor::new(ExtractorDescriptor::new("GenericIE", "Generic", "", true));
            let fallback = generic.extract_with_context(url, context)?;
            if let ExtractorResult::Single(mut info) = fallback {
                info.insert("id", serde_json::json!(video_id));
                info.insert("title", serde_json::json!(title));
                return Ok(ExtractorResult::single(info));
            }
            return Ok(fallback);
        }
        let first = formats.first().cloned().expect("WorldStar format");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
