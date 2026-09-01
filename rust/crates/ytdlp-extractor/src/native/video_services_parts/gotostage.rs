/// Native GoToStage registration and asset extractor.
pub struct GoToStageExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GoToStageExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GoToStageExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GoToStage URL has no ID")
            })?;
        let metadata_url = format!("https://api.gotostage.com/contents?ids={video_id}");
        let metadata = context
            .get_json(&metadata_url)?
            .as_array()
            .and_then(|values| values.first())
            .cloned()
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GoToStage video {video_id} has no metadata"),
                )
            })?;
        let registration_data = serde_json::json!({
            "product": json_string(&metadata, "product"),
            "resourceType": json_string(&metadata, "contentType"),
            "productReferenceKey": json_string(&metadata, "productRefKey"),
            "firstName": "foo",
            "lastName": "bar",
            "email": "foobar@example.com",
        });
        let registration = gotostage_registration(context, &registration_data)?;
        let registration_key = json_string(&registration, "registrationKey").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GoToStage registration for {video_id} has no key"),
            )
        })?;

        let mut asset_request =
            Request::new(format!("https://api.gotostage.com/contents/{video_id}/asset"));
        asset_request
            .headers_mut()
            .set("x-registrantkey", registration_key);
        let asset_response = context.request(&asset_request)?;
        let asset = serde_json::from_slice::<serde_json::Value>(asset_response.body()).map_err(
            |error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid GoToStage asset JSON for {video_id}: {error}"),
                )
            },
        )?;
        let media_url = json_string(&asset, "cdnLocation")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GoToStage video {video_id} has no CDN asset"),
                )
            })?;

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&metadata, "title"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert_if_some(
            "thumbnail",
            metadata
                .get("thumbnail")
                .and_then(|thumbnail| json_string(thumbnail, "location")),
        );
        info.insert_if_some("duration", json_f64(&metadata, "duration"));
        info.insert_if_some(
            "categories",
            json_string(&metadata, "category").map(|category| vec![category]),
        );
        info.insert("is_live", serde_json::json!(false));
        Ok(ExtractorResult::single(info))
    }
}

fn gotostage_registration(
    context: &ExtractionContext,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new("https://api-registrations.logmeininc.com/registrations");
    request.set_method("POST").map_err(map_request_error)?;
    request.headers_mut().set("Content-Type", "application/json");
    request.set_data(Some(serde_json::to_vec(payload).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("could not encode GoToStage registration: {error}"),
        )
    })?));
    let response = context.request_with_status(&request, &[409])?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid GoToStage registration JSON: {error}"),
        )
    })
}
