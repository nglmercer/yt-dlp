/// Native Katsomo asset/playback extractor.
pub struct KatsomoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KatsomoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KatsomoExtractor {
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
                    "Katsomo URL has no video ID",
                )
            })?;
        let asset = katsomo_asset(context, &video_id)?;
        let title = json_string(&asset, "subtitle")
            .filter(|value| !value.is_empty())
            .or_else(|| json_string(&asset, "title"))
            .unwrap_or(video_id.as_str())
            .to_owned();
        let is_live = json_bool(&asset, "live").unwrap_or(false);
        let formats = katsomo_formats(&video_id, is_live, context)?;
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let description = json_string(&asset, "description")
            .map(html_text_fragment)
            .filter(|value| !value.is_empty());
        let categories = json_string(&asset, "keywords").and_then(|value| {
            let categories = value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(serde_json::Value::from)
                .collect::<Vec<_>>();
            (!categories.is_empty()).then_some(serde_json::Value::Array(categories))
        });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnails", katsomo_thumbnail_list(asset.get("imageVersions")));
        info.insert_if_some(
            "timestamp",
            json_string(&asset, "createTime")
                .and_then(|value| parse_timestamp(value.to_owned())),
        );
        info.insert_if_some(
            "duration",
            json_f64(&asset, "accurateDuration").or_else(|| json_f64(&asset, "duration")),
        );
        info.insert_if_some("view_count", json_i64(&asset, "views"));
        info.insert_if_some("categories", categories);
        info.insert("is_live", serde_json::json!(is_live));
        info.insert_if_some("url", first_format.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first_format.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
