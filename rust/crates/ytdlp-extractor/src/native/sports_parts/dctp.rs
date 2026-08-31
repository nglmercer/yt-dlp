/// Native DCTP versioned REST/API extractor.
pub struct DctpTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DctpTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DctpTvExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DCTP URL has no slug")
            })?;
        let base_url = "http://dctp-ivms2-restapi.s3.amazonaws.com";
        let version = context.get_json(&format!("{base_url}/version.json"))?;
        let version_name = json_string(&version, "version_name").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "DCTP version response has no version_name",
            )
        })?;
        let restapi_base = format!("{base_url}/{version_name}/restapi");
        let info = context.get_json(&format!("{restapi_base}/slugs/{display_id}.json"))?;
        let object_id = json_value_string(info.get("object_id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DCTP film {display_id} has no object ID"),
            )
        })?;
        let media = context.get_json(&format!("{restapi_base}/media/{object_id}.json"))?;
        let uuid = json_string(&media, "uuid").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DCTP film {display_id} has no media UUID"),
            )
        })?;
        let title = json_string(&media, "title").unwrap_or(&display_id);
        let is_wide = json_bool(&media, "is_wide").unwrap_or(false);
        let mut formats = Vec::new();
        let mut add_formats = |suffix: &str| {
            let filename = format!("{uuid}_dctp_{suffix}.m4v");
            formats.push(serde_json::json!({
                "format_id": format!("hls-{suffix}"),
                "url": format!("https://cdn-segments.dctp.tv/{filename}/playlist.m3u8"),
                "protocol": "m3u8_native",
                "ext": "m4v",
            }));
            formats.push(serde_json::json!({
                "format_id": format!("s3-{suffix}"),
                "url": format!("https://completed-media.s3.amazonaws.com/{filename}"),
                "ext": "m4v",
            }));
            formats.push(serde_json::json!({
                "format_id": format!("http-{suffix}"),
                "url": format!("https://cdn-media.dctp.tv/{filename}"),
                "ext": "m4v",
            }));
        };
        add_formats(&format!("0500_{}", if is_wide { "16x9" } else { "4x3" }));
        if is_wide {
            add_formats("720p");
        }
        let thumbnails = media
            .get("images")
            .and_then(serde_json::Value::as_array)
            .map(|images| {
                serde_json::Value::Array(
                    images
                        .iter()
                        .filter_map(|image| {
                            let image_url = json_string(image, "url")?;
                            let mut thumbnail = serde_json::Map::new();
                            thumbnail.insert(
                                "url".to_owned(),
                                serde_json::Value::String(image_url.to_owned()),
                            );
                            if let Some(width) = json_i64(image, "width") {
                                thumbnail.insert("width".to_owned(), serde_json::json!(width));
                            }
                            if let Some(height) = json_i64(image, "height") {
                                thumbnail.insert("height".to_owned(), serde_json::json!(height));
                            }
                            Some(serde_json::Value::Object(thumbnail))
                        })
                        .collect(),
                )
            })
            .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()));
        let first = formats.first().cloned().expect("DCTP format");
        let mut result = InfoDict::new();
        result.insert("id", serde_json::json!(uuid));
        result.insert("display_id", serde_json::json!(display_id));
        result.insert("title", serde_json::json!(title));
        result.insert_if_some("alt_title", json_string(&media, "subtitle"));
        result.insert_if_some(
            "description",
            json_string(&media, "description").or_else(|| json_string(&media, "teaser")),
        );
        result.insert_if_some(
            "timestamp",
            json_string(&media, "created")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        result.insert_if_some(
            "duration",
            json_f64(&media, "duration_in_ms").map(|milliseconds| milliseconds / 1000.0),
        );
        result.insert_if_some("thumbnails", thumbnails);
        result.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        result.insert("ext", serde_json::json!("m4v"));
        result.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(result))
    }
}
