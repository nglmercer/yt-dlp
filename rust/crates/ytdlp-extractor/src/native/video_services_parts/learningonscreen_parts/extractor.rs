pub struct LearningOnScreenExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LearningOnScreenExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LearningOnScreenExtractor {
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
                    "Learning on Screen URL has no programme ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let formats = los_formats(url, &webpage);
        if formats.is_empty() {
            if webpage.contains("PHPSESSID-LOS-LIVE")
                || webpage.to_ascii_lowercase().contains("log in")
            {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: Learning on Screen programme {video_id} requires an authenticated session cookie"
                    ),
                ));
            }
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Learning on Screen programme {video_id} has no HTML5 media"),
            ));
        }
        let title = los_title(&webpage).unwrap_or_else(|| video_id.clone());
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Learning on Screen programme {video_id} has no usable media"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("duration", los_duration(&webpage));
        info.insert_if_some(
            "timestamp",
            los_broadcast_date(&webpage).and_then(|value| los_timestamp(&value)),
        );
        info.insert_if_some("thumbnail", los_poster(url, &webpage));
        info.insert_if_some("url", first.get("url").cloned());
        info.insert_if_some("ext", first.get("ext").cloned());
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
