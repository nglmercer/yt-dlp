/// Native A Galega Interactvty API/HLS extractor.
pub struct AGalegaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AGalegaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AGalegaExtractor {
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
                    "A Galega URL has no video ID",
                )
            })?;
        let token = agalega_access_token(context)?;
        let content_data = agalega_api_json(
            context,
            &format!("content/{video_id}/"),
            &token,
            "image,is_premium,short_description,has_subtitle",
        )
        .unwrap_or_else(|_| serde_json::json!({}));
        let resource_data = agalega_api_json(
            context,
            &format!("content_resources/{video_id}/"),
            &token,
            "media_url",
        )?;
        let streams = resource_data
            .get("results")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("A Galega video {video_id} has no resource list"),
                )
            })?;
        let mut formats = Vec::new();
        for stream in streams {
            let Some(stream_url) = json_string(stream, "media_url").filter(|value| {
                value.starts_with("http://") || value.starts_with("https://")
            }) else {
                continue;
            };
            let extension = yt_dlp_core::determine_ext(Some(stream_url), "unknown");
            if extension != "m3u8" {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: A Galega native extractor only implements HLS streams, got {stream_url}"
                    ),
                ));
            }
            formats.push(serde_json::json!({
                "url": stream_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        let first_format = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("A Galega video {video_id} has no HLS streams"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&content_data, "name"));
        info.insert_if_some(
            "description",
            json_string(&content_data, "description")
                .or_else(|| json_string(&content_data, "short_description")),
        );
        if let Some(image) = json_string(&content_data, "image").filter(|value| {
            value.starts_with("http://") || value.starts_with("https://")
        }) {
            info.insert("thumbnail", serde_json::json!(image));
        }
        info.insert_if_some("url", first_format.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first_format.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn agalega_access_token(context: &ExtractionContext) -> Result<String, ExtractorError> {
    let response = native_post_json(
        context,
        "https://www.agalega.gal/api/fetch-api/jwt/token",
        &serde_json::json!({
            "username": serde_json::Value::Null,
            "password": serde_json::Value::Null,
            "client": "crtvg",
            "checkExistsCookies": false,
        }),
    )?;
    json_string(&response, "access")
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "A Galega token response has no access token",
            )
        })
}

fn agalega_api_json(
    context: &ExtractionContext,
    endpoint: &str,
    token: &str,
    fields: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(format!(
        "https://api-agalega.interactvty.com/api/2.0/contents/{endpoint}"
    ));
    request.update_query(&[("optional_fields".to_owned(), fields.to_owned())]);
    request
        .headers_mut()
        .set("Authorization", format!("jwtok {token}"));
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid A Galega API JSON from {endpoint}: {error}"),
        )
    })
}
