/// Native Harpodeon page extractor for deterministic MP4 URLs.
pub struct HarpodeonExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HarpodeonExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HarpodeonExtractor {
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
                    "Harpodeon URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());

        let metadata_matcher = Regex::new(
            r##"(?is)<div[^>]+videoInfo[^<]*<h2[^>]*>(.*?)</h2>(?:\s*<p[^>]*>\(([^,]+),\s*)?(\d{4})?"##,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Harpodeon metadata matcher: {error}"),
            )
        })?;
        let metadata = metadata_matcher.captures(&webpage).ok().flatten();
        let title = metadata
            .as_ref()
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty());
        let creator = metadata
            .as_ref()
            .and_then(|captures| captures.get(2))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty());
        let release_year = metadata
            .as_ref()
            .and_then(|captures| captures.get(3))
            .and_then(|value| value.as_str().parse::<i64>().ok());

        let hp_base = Regex::new(r##"(?i)hpBase\(\s*["']([^"']+)"##)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Harpodeon video {video_id} has no hpBase URL"),
                )
            })?;
        let injection_matcher = Regex::new(
            r##"(?i)hpInjectVideo\(\s*["'](?P<video>\w+)["']\s*,\s*["'](?P<resolution>\d+)["']"##,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Harpodeon injection matcher: {error}"),
            )
        })?;
        let injection = injection_matcher
            .captures(&webpage)
            .ok()
            .flatten()
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Harpodeon video {video_id} has no injected media parameters"),
                )
            })?;
        let injected_video = injection
            .name("video")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Harpodeon video {video_id} has no injected media ID"),
                )
            })?;
        let resolution = injection
            .name("resolution")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Harpodeon video {video_id} has no injected resolution"),
                )
            })?;
        let media_url = format!("{hp_base}{injected_video}_{resolution}.mp4");

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "protocol": "http",
                "ext": "mp4",
            }]),
        );
        info.insert(
            "http_headers",
            serde_json::json!({"Referer": url}),
        );
        info.insert_if_some(
            "description",
            html_meta_value(&webpage, "description")
                .map(|value| html_text_fragment(&value))
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some("creator", creator);
        info.insert_if_some("release_year", release_year);
        Ok(ExtractorResult::single(info))
    }
}
