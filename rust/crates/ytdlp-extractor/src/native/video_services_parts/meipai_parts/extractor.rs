pub struct MeipaiExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MeipaiExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MeipaiExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Meipai URL has no media ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let formats = meipai_media_formats(url, &webpage, &video_id)?;
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            html_meta_value(&webpage, "og:title").or_else(|| html_title_value(&webpage)),
        );
        info.insert_if_some("description", html_meta_value(&webpage, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        info.insert_if_some(
            "duration",
            meipai_meta_duration(&webpage),
        );
        info.insert_if_some(
            "timestamp",
            html_meta_value(&webpage, "video:release_date").and_then(parse_timestamp),
        );
        info.insert_if_some("view_count", meipai_meta_number(&webpage, "interactionCount"));
        info.insert_if_some("creator", html_meta_value(&webpage, "video:director"));
        info.insert_if_some("tags", meipai_tags(&webpage));
        info.insert("url", first_url);
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
