/// Native CloudyCDN player API/HLS extractor.
pub struct CloudyCdnExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CloudyCdnExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CloudyCdnExtractor {
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
                "CloudyCDN URL did not match its native pattern",
            )
        })?;
        let domain = captures
            .name("domain")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CloudyCDN URL has no domain")
            })?;
        let site_id = captures
            .name("site_id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "CloudyCDN URL has no site ID",
                )
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CloudyCDN URL has no media ID")
            })?;
        let player_url = format!(
            "https://player.{domain}/player/{site_id}/media/{video_id}/"
        );
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("version", "6.4.0");
        form.append_pair("referer", url);
        let mut request = Request::new(player_url);
        request.set_method("POST").map_err(map_request_error)?;
        request.set_data(Some(form.finish().into_bytes()));
        let response = context.request(&request)?;
        let data: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid CloudyCDN player data for {video_id}: {error}"),
            )
        })?;
        let sources = data
            .get("source")
            .and_then(|source| source.get("sources"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("CloudyCDN media {video_id} has no source list"),
                )
            })?;
        let mut formats = Vec::new();
        for (index, source) in sources.iter().enumerate() {
            let Some(source_url) = json_string(source, "src").filter(|value| {
                value.starts_with("http://") || value.starts_with("https://")
            }) else {
                continue;
            };
            let extension = yt_dlp_core::determine_ext(Some(source_url), "unknown");
            if extension != "m3u8" {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: CloudyCDN native extractor only implements HLS sources, got {source_url}"
                    ),
                ));
            }
            let mut format = serde_json::Map::new();
            format.insert("url".to_owned(), serde_json::json!(source_url));
            format.insert(
                "format_id".to_owned(),
                serde_json::json!(if index == 0 {
                    "hls".to_owned()
                } else {
                    format!("hls-{index}")
                }),
            );
            format.insert("ext".to_owned(), serde_json::json!("mp4"));
            format.insert("protocol".to_owned(), serde_json::json!("m3u8_native"));
            if source_url.contains("_vo_") {
                format.insert("acodec".to_owned(), serde_json::json!("none"));
            }
            formats.push(serde_json::Value::Object(format));
        }
        let first_format = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CloudyCDN media {video_id} has no playable HLS sources"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&data, "name"));
        info.insert_if_some("duration", json_i64(&data, "duration"));
        info.insert_if_some(
            "timestamp",
            json_string(&data, "upload_date")
                .and_then(|value| parse_timestamp(value.to_owned())),
        );
        info.insert_if_some(
            "thumbnail",
            data.get("source")
                .and_then(|source| json_string(source, "poster")),
        );
        info.insert_if_some("url", first_format.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first_format.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
