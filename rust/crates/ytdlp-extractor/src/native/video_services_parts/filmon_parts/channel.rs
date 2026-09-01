/// Native FilmOn channel/VOD-channel API extractor.
pub struct FilmOnChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FilmOnChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FilmOnChannelExtractor {
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
        let requested_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "FilmOn channel has no ID")
            })?;
        let response = context.get_json(&format!(
            "http://www.filmon.com/api-v2/channel/{requested_id}"
        ))?;
        let channel = response.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FilmOn channel {requested_id} API response has no data"),
            )
        })?;
        let channel_id = json_value_string(channel.get("id")).unwrap_or(requested_id.clone());
        let title = json_string(channel, "title")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FilmOn channel {channel_id} has no title"),
                )
            })?;
        let is_live =
            !json_bool(channel, "is_vod").unwrap_or(false) && !json_bool(channel, "is_vox").unwrap_or(false);
        let (formats, unsupported_streams) = filmon_channel_formats(channel.get("streams"));
        if formats.is_empty() && unsupported_streams {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: FilmOn channel {channel_id} exposes only Wowza/RTMP streams; \
                     native fragmented-stream support is not implemented"
                ),
            ));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FilmOn channel {channel_id} has no playable streams"),
            ));
        }
        let mut thumbnails = Vec::new();
        for (id, width, height) in [
            ("logo", 56, 28),
            ("big_logo", 106, 106),
            ("extra_big_logo", 300, 300),
        ] {
            thumbnails.push(serde_json::json!({
                "id": id,
                "url": format!("http://static.filmon.com/assets/channels/{channel_id}/{id}.png"),
                "width": width,
                "height": height,
            }));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        info.insert_if_some("display_id", json_string(channel, "alias"));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", json_string(channel, "description"));
        info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("is_live", serde_json::json!(is_live));
        Ok(ExtractorResult::single(info))
    }
}
