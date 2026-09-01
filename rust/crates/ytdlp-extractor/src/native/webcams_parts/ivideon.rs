/// Native Ivideon TV live-camera extractor.
pub struct IvideonExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl IvideonExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for IvideonExtractor {
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
                "Ivideon URL did not match its native pattern",
            )
        })?;
        let server_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Ivideon URL has no server ID")
            })?;
        let camera_id = captures
            .name("camera_id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Ivideon URL has no camera ID",
                )
            })?;
        let camera_url = resolve_url(url, &format!("/tv/camera/{server_id}/{camera_id}/"));
        let webpage = context
            .get(&camera_url)
            .ok()
            .map(|response| String::from_utf8_lossy(response.body()).into_owned());
        let (camera_name, description) = webpage
            .as_deref()
            .and_then(ivideon_config_metadata)
            .unwrap_or_default();
        let camera_name = camera_name.or_else(|| {
            webpage.as_deref().and_then(|html| {
                html_meta_value(html, "name").or_else(|| ivideon_heading(html))
            })
        });
        let formats = ["low", "mid", "hi"]
            .into_iter()
            .enumerate()
            .map(|(quality, format_id)| {
                let mut stream_url =
                    url::Url::parse("https://streaming.ivideon.com/flv/live").unwrap();
                stream_url
                    .query_pairs_mut()
                    .append_pair("server", &server_id)
                    .append_pair("camera", &camera_id)
                    .append_pair("sessionId", "demo")
                    .append_pair("q", format_id);
                serde_json::json!({
                    "url": stream_url.to_string(),
                    "format_id": format_id,
                    "ext": "flv",
                    "quality": quality as i64,
                })
            })
            .collect::<Vec<_>>();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(server_id));
        info.insert(
            "title",
            serde_json::json!(camera_name.unwrap_or_else(|| server_id.clone())),
        );
        info.insert_if_some("description", description);
        info.insert("is_live", serde_json::json!(true));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn ivideon_config_metadata(webpage: &str) -> Option<(Option<String>, Option<String>)> {
    let config = json_object_after_marker(webpage, "var config")?;
    let camera = config
        .get("ivTvAppOptions")
        .and_then(|options| options.get("currentCameraInfo"))?;
    let name = json_string(camera, "camera_name").map(str::to_owned);
    let description = camera
        .get("misc")
        .and_then(|misc| json_string(misc, "description"))
        .map(str::to_owned);
    Some((name, description))
}

fn ivideon_heading(webpage: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<h1\b[^>]*\bclass\s*=\s*["'][^"']*\bb-video-title\b[^"']*["'][^>]*>(.*?)</h1\s*>"#,
    )
    .ok()?;
    matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.trim().is_empty())
}
