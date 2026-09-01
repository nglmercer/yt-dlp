/// Native Canal Sur Más Interactvty API extractor.
pub struct CanalsurmasExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CanalsurmasExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CanalsurmasExtractor {
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
                    "Canal Sur Más URL has no video ID",
                )
            })?;
        let token = canalsurmas_access_token(context)?;
        let video_info = canalsurmas_api_json(
            context,
            &format!("content/{video_id}/"),
            &token,
            &["description", "image", "duration", "created_at", "tags"],
        )?;
        let stream_info = canalsurmas_api_json(
            context,
            &format!("content_resources/{video_id}/"),
            &token,
            &["media_url"],
        )?;
        let mut formats = Vec::new();
        let streams = stream_info
            .get("results")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Canal Sur Más video {video_id} has no resource list"),
                )
            })?;
        for stream in streams {
            let Some(stream_url) = json_string(stream, "media_url").filter(|value| !value.is_empty())
            else {
                continue;
            };
            let extension = yt_dlp_core::determine_ext(Some(stream_url), "unknown");
            if matches!(extension.as_str(), "f4m" | "smil")
                || stream_url.starts_with("rtmp://")
                || stream_url.starts_with("rtmps://")
            {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: Canal Sur Más native extractor does not implement stream {stream_url}"
                    ),
                ));
            }
            let is_hls = extension == "m3u8";
            let mut format = serde_json::Map::new();
            format.insert("url".to_owned(), serde_json::json!(stream_url));
            format.insert("protocol".to_owned(), serde_json::json!(if is_hls {
                "m3u8_native"
            } else {
                "http"
            }));
            if is_hls {
                format.insert("format_id".to_owned(), serde_json::json!("hls"));
                format.insert("ext".to_owned(), serde_json::json!("mp4"));
            } else if extension != "unknown" {
                format.insert("ext".to_owned(), serde_json::json!(extension));
            }
            formats.push(serde_json::Value::Object(format));
        }
        let first_format = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Canal Sur Más video {video_id} has no playable streams"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&video_info, "name"));
        info.insert_if_some("description", json_string(&video_info, "description"));
        if let Some(image) = json_string(&video_info, "image").filter(|value| {
            value.starts_with("http://") || value.starts_with("https://")
        }) {
            info.insert("thumbnail", serde_json::json!(image));
        }
        info.insert_if_some(
            "duration",
            json_f64(&video_info, "duration"),
        );
        info.insert_if_some(
            "timestamp",
            json_string(&video_info, "created_at")
                .and_then(|value| parse_timestamp(value.to_owned())),
        );
        if let Some(tags) = video_info.get("tags").and_then(serde_json::Value::as_array) {
            let tags = tags
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>();
            info.insert_if_some("tags", (!tags.is_empty()).then_some(tags));
        }
        info.insert_if_some("url", first_format.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first_format.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn canalsurmas_access_token(
    context: &ExtractionContext,
) -> Result<String, ExtractorError> {
    let response = native_post_json(
        context,
        "https://api-rtva.interactvty.com/jwt/token/",
        &serde_json::json!({
            "username": "canalsur_demo",
            "password": "dsUBXUcI",
        }),
    )?;
    json_string(&response, "access")
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Canal Sur Más token response has no access token",
            )
        })
}

fn canalsurmas_api_json(
    context: &ExtractionContext,
    endpoint: &str,
    token: &str,
    fields: &[&str],
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(format!(
        "https://api-rtva.interactvty.com/api/2.0/contents/{endpoint}"
    ));
    request.update_query(&[("optional_fields".to_owned(), fields.join(","))]);
    request
        .headers_mut()
        .set("Authorization", format!("jwtok {token}"));
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Canal Sur Más API JSON from {endpoint}: {error}"),
        )
    })
}
