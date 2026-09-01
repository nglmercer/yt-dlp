/// Native Fox Sports API/preplay/HLS extractor.
pub struct FoxSportsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FoxSportsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FoxSportsExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Fox Sports URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let webpage = String::from_utf8_lossy(webpage.body());
        let json_ld = html_json_ld(&webpage).unwrap_or_else(|| serde_json::json!({}));

        let api_url = format!("https://api3.fox.com/v2.0/vodplayer/sportsclip/{video_id}");
        let mut api_request = Request::new(api_url);
        api_request
            .headers_mut()
            .set("x-api-key", "cf289e299efdfa39fb6316f259d1de93");
        let api_response = context.request(&api_request)?;
        let data = serde_json::from_slice::<serde_json::Value>(api_response.body()).map_err(
            |error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Fox Sports API JSON for {video_id}: {error}"),
                )
            },
        )?;
        let source_url = json_string(&data, "url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Fox Sports video {video_id} has no preplay URL"),
                )
            })?;
        let mut head_request = Request::new(source_url);
        head_request.set_method("HEAD").map_err(map_request_error)?;
        let preplay_url = context.request(&head_request)?.url().to_owned();
        let path = uplynk_preplay_path(&preplay_url).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Fox Sports video {video_id} returned a non-Uplynk preplay URL"
                ),
            )
        })?;
        let preplay = context.get_json(&preplay_url)?;
        let session_id = json_value_string(preplay.get("sid"));
        let mut info = uplynk_content_info(
            context,
            &path,
            session_id.as_deref(),
            Some("https://www.foxsports.com"),
        )?;
        info.insert("display_id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            json_string(&data, "name")
                .or_else(|| json_string(&json_ld, "name"))
                .map(str::to_owned),
        );
        info.insert_if_some(
            "description",
            json_string(&data, "description")
                .or_else(|| json_string(&json_ld, "description"))
                .map(str::to_owned),
        );
        info.insert_if_some("duration", json_f64(&data, "durationInSeconds"));
        info.insert_if_some("timestamp", foxsports_timestamp(&json_ld));
        info.insert_if_some("thumbnails", foxsports_thumbnails(&json_ld));
        Ok(ExtractorResult::single(info))
    }
}

fn foxsports_timestamp(json_ld: &serde_json::Value) -> Option<i64> {
    json_i64(json_ld, "timestamp").or_else(|| {
        json_string(json_ld, "uploadDate")
            .map(str::to_owned)
            .and_then(parse_timestamp)
    })
}

fn foxsports_thumbnails(json_ld: &serde_json::Value) -> Option<Vec<serde_json::Value>> {
    let value = json_ld
        .get("thumbnailUrl")
        .or_else(|| json_ld.get("thumbnail"))?;
    let values = match value {
        serde_json::Value::Array(values) => values.iter().collect::<Vec<_>>(),
        _ => vec![value],
    };
    let thumbnails = values
        .into_iter()
        .filter_map(|value| {
            let url = value
                .as_str()
                .or_else(|| json_string(value, "url"))
                .filter(|url| url.starts_with("http://") || url.starts_with("https://"))?;
            Some(serde_json::json!({"url": url}))
        })
        .collect::<Vec<_>>();
    (!thumbnails.is_empty()).then_some(thumbnails)
}
