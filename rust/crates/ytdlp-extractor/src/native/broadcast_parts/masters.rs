/// Native Masters tournament video API extractor.
pub struct MastersExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MastersExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MastersExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Masters URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Masters URL has no ID")
            })?;
        let date = captures
            .name("date")
            .map(|value| value.as_str().replace('-', ""))
            .unwrap_or_default();
        let data = context.get_json(&format!(
            "https://www.masters.com/relatedcontent/rest/v2/masters_v1/en/content/masters_v1_{video_id}_en"
        ))?;
        let media_url = data
            .get("media")
            .and_then(|value| json_string(value, "m3u8"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Masters video {video_id} has no HLS URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert("upload_date", serde_json::json!(date));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        if let Some(images) = data
            .get("images")
            .and_then(|value| value.get(0))
            .and_then(serde_json::Value::as_object)
        {
            let thumbnails = images
                .iter()
                .filter_map(|(id, value)| {
                    Some(serde_json::json!({
                        "id": id,
                        "url": value.as_str()?
                    }))
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}
