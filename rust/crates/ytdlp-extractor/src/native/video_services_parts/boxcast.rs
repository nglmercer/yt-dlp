/// Native BoxCast broadcast preload/API extractor.
pub struct BoxCastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BoxCastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BoxCastExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "BoxCast URL has no broadcast ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let preload = json_object_after_marker(&webpage, "BOXCAST_PRELOAD")
            .unwrap_or_else(|| serde_json::json!({}));
        let broadcast = preload
            .get("broadcast")
            .and_then(|broadcast| broadcast.get("data"))
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| {
                context.get_json(&format!("https://api.boxcast.com/broadcasts/{display_id}"))
            })?;
        let view = preload
            .get("view")
            .and_then(|view| view.get("data"))
            .cloned()
            .or_else(|| {
                context
                    .get_json(&format!("https://api.boxcast.com/broadcasts/{display_id}/view"))
                    .ok()
            })
            .unwrap_or_else(|| serde_json::json!({}));
        if json_string(&view, "status") != Some("recorded") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: BoxCast native extractor does not implement non-recorded broadcast {display_id}"
                ),
            ));
        }
        let playlist = json_string(&view, "playlist")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("BoxCast broadcast {display_id} has no HLS playlist"),
                )
            })?;
        let formats = serde_json::json!([{
            "url": playlist,
            "format_id": "hls",
            "protocol": "m3u8_native",
            "ext": "mp4",
        }]);
        let broadcast_id = json_value_string(broadcast.get("id"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| display_id.clone());
        let title = json_string(&broadcast, "name")
            .map(|value| html_text_fragment(value))
            .filter(|value| !value.is_empty())
            .or_else(|| boxcast_meta_value(&webpage, "og:title"));
        let description = json_string(&broadcast, "description")
            .map(|value| html_text_fragment(value))
            .filter(|value| !value.is_empty())
            .or_else(|| boxcast_meta_value(&webpage, "og:description"));
        let thumbnail = json_string(&broadcast, "preview")
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| boxcast_meta_value(&webpage, "og:image"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(broadcast_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some(
            "release_timestamp",
            json_string(&broadcast, "streamed_at")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("uploader", json_string(&broadcast, "account_name"));
        info.insert_if_some("uploader_id", json_string(&broadcast, "account_id"));
        info.insert("url", serde_json::json!(playlist));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", formats);
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn boxcast_meta_value(html: &str, key: &str) -> Option<String> {
    html_meta_value(html, key)
        .map(|value| html_text_fragment(&value))
        .filter(|value| !value.is_empty())
}
