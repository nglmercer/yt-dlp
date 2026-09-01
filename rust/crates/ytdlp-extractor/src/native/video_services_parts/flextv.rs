/// Native FlexTV/TtingLive channel API extractor.
pub struct FlexTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FlexTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FlexTvExtractor {
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
        let channel_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "FlexTV URL has no channel ID",
                )
            })?;
        let mut request = Request::new(format!(
            "https://api.ttinglive.com/api/channels/{channel_id}/stream"
        ));
        request.update_query(&[("option".to_owned(), "all".to_owned())]);
        let response = context.request_with_status(&request, &[400])?;
        if response.status() == 400 {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FlexTV channel {channel_id} is not live"),
            ));
        }
        let stream_data = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(
            |error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid FlexTV stream JSON for {channel_id}: {error}"),
                )
            },
        )?;
        let mut formats = Vec::new();
        if let Some(sources) = stream_data.get("sources").and_then(serde_json::Value::as_array) {
            for source in sources {
                if json_string(source, "format") == Some("ivs") {
                    if let Some(media_url) = json_string(source, "url") {
                        flextv_add_format(
                            &mut formats,
                            media_url,
                            "ivs",
                            None,
                            "mp4",
                            "m3u8_native",
                        );
                    }
                }
                for format_type in ["hls", "flv"] {
                    let Some(resolutions) = source
                        .get("urlDetail")
                        .and_then(|detail| detail.get(format_type))
                        .and_then(|detail| detail.get("resolution"))
                    else {
                        continue;
                    };
                    for data in json_object_values(resolutions) {
                        let Some(media_url) = json_string(data, "url") else {
                            continue;
                        };
                        let suffix = json_string(data, "suffixName").unwrap_or("");
                        let format_id = format!("{format_type}{suffix}");
                        let height = json_i64(data, "resolution");
                        flextv_add_format(
                            &mut formats,
                            media_url,
                            &format_id,
                            height,
                            if format_type == "hls" { "mp4" } else { "flv" },
                            if format_type == "hls" {
                                "m3u8_native"
                            } else {
                                "http"
                            },
                        );
                    }
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FlexTV channel {channel_id} has no playable stream URLs"),
            ));
        }
        let stream = stream_data
            .get("stream")
            .unwrap_or(&serde_json::Value::Null);
        let owner = stream_data
            .get("owner")
            .unwrap_or(&serde_json::Value::Null);
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        info.insert_if_some("title", json_string(stream, "title"));
        info.insert_if_some(
            "timestamp",
            json_string(stream, "createdAt")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("thumbnail", json_string(&stream_data, "thumbUrl"));
        info.insert_if_some("channel", json_string(owner, "name"));
        info.insert_if_some("channel_id", json_value_string(owner.get("id")));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("is_live", serde_json::json!(true));
        Ok(ExtractorResult::single(info))
    }
}

fn flextv_add_format(
    formats: &mut Vec<serde_json::Value>,
    media_url: &str,
    format_id: &str,
    height: Option<i64>,
    ext: &str,
    protocol: &str,
) {
    if !media_url.starts_with("http://") && !media_url.starts_with("https://") {
        return;
    }
    if formats.iter().any(|format| {
        format.get("url").and_then(serde_json::Value::as_str) == Some(media_url)
    }) {
        return;
    }
    let mut format = serde_json::json!({
        "format_id": format_id,
        "url": media_url,
        "ext": ext,
        "protocol": protocol,
        "is_live": true,
    });
    if let Some(height) = height {
        format["height"] = serde_json::json!(height);
    }
    formats.push(format);
}
