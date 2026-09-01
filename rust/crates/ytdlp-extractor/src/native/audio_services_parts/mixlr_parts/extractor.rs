/// Native Mixlr event API/audio extractor.
pub struct MixlrExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MixlrExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MixlrExtractor {
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
                "Mixlr event URL did not match its native pattern",
            )
        })?;
        let username = captures
            .name("username")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mixlr event has no username")
            })?;
        let event_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mixlr event has no ID")
            })?;
        let payload = mixlr_api(context, &username, "events", &event_id)?;
        let data = mixlr_data_attributes(&payload);
        let included = mixlr_included_attributes(&payload);
        let format_url = mixlr_attribute_string(data, included, "progressive_stream_url")
            .and_then(|value| mixlr_http_url(Some(value)));
        let formats = format_url
            .as_deref()
            .and_then(|value| mixlr_progressive_format(context, value))
            .into_iter()
            .collect::<Vec<_>>();
        if formats.is_empty() {
            let schedule = mixlr_attribute_string(data, included, "starts_at")
                .map(|value| format!("; scheduled start: {value}"))
                .unwrap_or_default();
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Mixlr event {event_id} has no available audio stream{schedule}"),
            ));
        }

        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let is_live = mixlr_attribute_bool(data, included, "live");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(event_id));
        info.insert("uploader", serde_json::json!(username));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some(
            "release_timestamp",
            mixlr_attribute_string(data, included, "starts_at")
                .and_then(parse_timestamp),
        );
        info.insert_if_some("title", mixlr_attribute_string(data, included, "title"));
        info.insert_if_some(
            "description",
            mixlr_attribute_string(data, included, "description"),
        );
        info.insert_if_some(
            "timestamp",
            mixlr_attribute_string(data, included, "started_at")
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "view_count",
            mixlr_attribute_i64(data, included, "concurrent_view_count"),
        );
        info.insert_if_some(
            "like_count",
            mixlr_attribute_i64(data, included, "heart_count"),
        );
        info.insert_if_some("is_live", is_live);
        if let Some(is_live) = is_live {
            info.insert(
                "live_status",
                serde_json::json!(if is_live { "is_live" } else { "not_live" }),
            );
        }
        info.insert_if_some(
            "thumbnail",
            mixlr_attribute_string(data, included, "artwork_url"),
        );
        info.insert_if_some(
            "uploader_id",
            mixlr_attribute_string(data, included, "broadcaster_id"),
        );
        info.insert(
            "url",
            first
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp3")),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Mixlr recording API/audio extractor.
pub struct MixlrRecoringExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MixlrRecoringExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MixlrRecoringExtractor {
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
                "Mixlr recording URL did not match its native pattern",
            )
        })?;
        let username = captures
            .name("username")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Mixlr recording has no username",
                )
            })?;
        let recording_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mixlr recording has no ID")
            })?;
        let payload = mixlr_api(context, &username, "recordings", &recording_id)?;
        let data = mixlr_data_attributes(&payload);
        let null = serde_json::Value::Null;
        let media_url = mixlr_http_url(mixlr_attribute_string(data, &null, "url")).ok_or_else(
            || {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Mixlr recording {recording_id} has no playable audio URL"),
                )
            },
        )?;
        let extension = mixlr_attribute_string(data, &null, "file_format")
            .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(&media_url), "mp3"));
        let format = serde_json::json!({
            "url": media_url,
            "format_id": "source",
            "ext": extension,
            "vcodec": "none",
        });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(recording_id));
        info.insert("ext", serde_json::json!(extension));
        info.insert("url", serde_json::json!(media_url));
        info.insert("formats", serde_json::json!([format]));
        info.insert_if_some("title", mixlr_attribute_string(data, &null, "title"));
        info.insert_if_some(
            "description",
            mixlr_attribute_string(data, &null, "description"),
        );
        info.insert_if_some(
            "timestamp",
            mixlr_attribute_string(data, &null, "created_at").and_then(parse_timestamp),
        );
        info.insert_if_some(
            "duration",
            mixlr_attribute_i64(data, &null, "duration"),
        );
        info.insert_if_some(
            "thumbnail",
            mixlr_attribute_string(data, &null, "artwork_url"),
        );
        info.insert_if_some(
            "uploader_id",
            mixlr_attribute_string(data, &null, "user_id"),
        );
        Ok(ExtractorResult::single(info))
    }
}
