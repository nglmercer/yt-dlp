/// Native Cineteca Milano catalog/API HLS extractor.
pub struct CinetecaMilanoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CinetecaMilanoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CinetecaMilanoExtractor {
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
                    "Cineteca Milano URL has no film ID",
                )
            })?;
        let api_url = format!("https://www.cinetecamilano.it/api/catalogo/{video_id}/?");
        let mut request = Request::new(&api_url);
        request.headers_mut().set("Referer", url);
        request.headers_mut().set(
            "Authorization",
            cineteca_authorization(context).unwrap_or_default(),
        );
        let response = context.request(&request)?;
        let film_json: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Cineteca Milano API response for {video_id}: {error}"),
            )
        })?;
        if film_json.get("success").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Cineteca Milano film {video_id} information was not found"),
            ));
        }
        let archive = film_json.get("archive").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Cineteca Milano film {video_id} has no archive data"),
            )
        })?;
        let media_url = archive
            .get("drm")
            .and_then(|drm| json_string(drm, "hls"))
            .map(|value| resolve_url(url, value))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Cineteca Milano film {video_id} has no HLS URL"),
                )
            })?;
        let thumbnail = archive
            .get("thumb")
            .and_then(|thumb| json_string(thumb, "src"))
            .map(|value| resolve_url(url, &value.replace("/public/", "/storage/")));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(archive, "title"));
        info.insert_if_some(
            "description",
            json_string(archive, "description")
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "duration",
            json_f64(archive, "duration").map(|minutes| minutes * 60.0),
        );
        info.insert_if_some(
            "release_timestamp",
            cineteca_timestamp(archive, "updated_at"),
        );
        info.insert_if_some(
            "modified_timestamp",
            cineteca_timestamp(archive, "created_at"),
        );
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

fn cineteca_authorization(context: &ExtractionContext) -> Option<String> {
    let cookies = context
        .cookie_jar()
        .lock()
        .ok()?
        .cookie_header("https://www.cinetecamilano.it/")
        .ok()??;
    cookies
        .split(';')
        .find_map(|cookie| cookie.trim().strip_prefix("cnt-token="))
        .filter(|value| !value.is_empty())
        .map(|value| format!("Bearer {value}"))
}

fn cineteca_timestamp(archive: &serde_json::Value, key: &str) -> Option<i64> {
    let value = json_string(archive, key)?;
    parse_timestamp(value.to_owned()).or_else(|| parse_timestamp(value.replace(' ', "T")))
}
