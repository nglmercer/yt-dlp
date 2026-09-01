pub struct MagellanTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MagellanTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MagellanTvExtractor {
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
                    "MagellanTV URL has no video ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let next_data = html_script_json(&webpage, "__NEXT_DATA__")?;
        let react_context = magellantv_react_context(&next_data, &video_id)?;
        let data = magellantv_video_data(react_context, &video_id)?;
        let formats = magellantv_formats(data, &video_id)?;
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", first_url);
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("title", json_string(data, "title"));
        info.insert_if_some(
            "description",
            data.get("metadata")
                .and_then(|metadata| json_string(metadata, "description")),
        );
        info.insert_if_some("duration", magellantv_duration(data));
        info.insert_if_some("age_limit", magellantv_age_limit(data));
        if let Some(tags) = data
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|tags| !tags.is_empty())
        {
            info.insert("tags", serde_json::Value::Array(
                tags.into_iter().map(serde_json::Value::String).collect(),
            ));
        }
        Ok(ExtractorResult::single(info))
    }
}
