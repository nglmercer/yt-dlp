/// Native BongaCams live-room API/HLS extractor.
pub struct BongaCamsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BongaCamsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BongaCamsExtractor {
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
                "BongaCams URL did not match its native pattern",
            )
        })?;
        let host = captures
            .name("host")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "BongaCams URL has no host")
            })?;
        let channel_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "BongaCams URL has no channel ID",
                )
            })?;
        let room_data = bongacams_room_data(context, &host, &channel_id)?;
        let server_url = room_data
            .get("localData")
            .and_then(|local_data| json_string(local_data, "videoServerUrl"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("BongaCams room {channel_id} has no video server URL"),
                )
            })?;
        let performer = room_data.get("performerData");
        let uploader_id = performer
            .and_then(|performer| json_string(performer, "username"))
            .filter(|value| !value.is_empty())
            .unwrap_or(&channel_id)
            .to_owned();
        let uploader = performer
            .and_then(|performer| json_string(performer, "displayName"))
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let title = uploader.clone().unwrap_or_else(|| uploader_id.clone());
        let like_count = performer.and_then(|performer| json_i64(performer, "loversCount"));
        let hls_url = format!(
            "{}/hls/stream_{uploader_id}/playlist.m3u8",
            server_url.trim_end_matches('/')
        );
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("uploader", uploader);
        info.insert("uploader_id", serde_json::json!(uploader_id));
        info.insert_if_some("like_count", like_count);
        info.insert("age_limit", serde_json::json!(18));
        info.insert("is_live", serde_json::json!(true));
        info.insert("url", serde_json::json!(hls_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": hls_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
                "live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

fn bongacams_room_data(
    context: &ExtractionContext,
    host: &str,
    channel_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("https://{host}/tools/amf.php");
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("method", "getRoomData");
    form.append_pair("args[]", channel_id);
    form.append_pair("args[]", "false");

    let mut request = Request::new(endpoint);
    request.set_method("POST").map_err(map_request_error)?;
    request
        .headers_mut()
        .set("X-Requested-With", "XMLHttpRequest");
    request.headers_mut().set("Accept", "application/json");
    request.set_data(Some(form.finish().into_bytes()));
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid BongaCams room data for {channel_id}: {error}"),
        )
    })
}
