/// Native Livestreamfails API/direct-media extractor.
pub struct LivestreamfailsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LivestreamfailsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LivestreamfailsExtractor {
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
                    "Livestreamfails URL has no ID",
                )
            })?;
        let data = context.get_json(&format!("https://api.livestreamfails.com/clip/{video_id}"))?;
        let source_id = json_string(&data, "sourceId").map(str::to_owned);
        let remote_id = json_string(&data, "videoId").unwrap_or(&video_id);
        let media_url = format!("https://livestreamfails-video-prod.b-cdn.net/video/{remote_id}");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("display_id", source_id);
        info.insert_if_some("title", json_string(&data, "label").map(str::to_owned));
        info.insert_if_some(
            "creator",
            data.get("streamer")
                .and_then(|value| json_string(value, "label")),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&data, "imageId")
                .map(|value| format!("https://livestreamfails-image-prod.b-cdn.net/image/{value}")),
        );
        info.insert_if_some(
            "timestamp",
            json_string(&data, "createdAt")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
