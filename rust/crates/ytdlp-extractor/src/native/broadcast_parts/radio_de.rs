/// Native radio.de station-page extractor. The source marks this service as
/// non-working today, but its historical contract is still represented here
/// without a compatibility runtime.
pub struct RadioDeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RadioDeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RadioDeExtractor {
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
        let radio_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "radio.de URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let broadcast = json_object_after_marker(&html, "stationService")
            .and_then(|service| service.get("station").cloned())
            .or_else(|| json_object_after_marker(&html, "station"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("radio.de station {radio_id} has no broadcast data"),
                )
            })?;
        let stream_urls = broadcast
            .get("streamUrls")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("radio.de station {radio_id} has no streams"),
                )
            })?;
        let mut formats = Vec::new();
        for stream in stream_urls {
            let Some(stream_url) = json_string(stream, "streamUrl") else {
                continue;
            };
            let codec = json_string(stream, "streamContentFormat").unwrap_or("mp3");
            formats.push(serde_json::json!({
                "url": stream_url,
                "ext": codec.to_ascii_lowercase(),
                "acodec": codec,
                "abr": json_f64(stream, "bitRate"),
                "asr": json_f64(stream, "sampleRate"),
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("radio.de station {radio_id} has no playable streams"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(radio_id));
        info.insert(
            "title",
            serde_json::json!(json_string(&broadcast, "name").unwrap_or("radio.de station")),
        );
        info.insert_if_some(
            "description",
            json_string(&broadcast, "description")
                .or_else(|| json_string(&broadcast, "shortDescription")),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&broadcast, "picture4Url")
                .or_else(|| json_string(&broadcast, "picture4TransUrl"))
                .or_else(|| json_string(&broadcast, "logo100x100")),
        );
        info.insert("is_live", serde_json::json!(true));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
