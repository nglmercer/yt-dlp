/// Native Faulio VOD page/API/HLS-DASH extractor.
pub struct FaulioExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FaulioExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FaulioExtractor {
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
        let video_id = faulio_match_id(&self.matcher, url, "Faulio")?;
        let api_base = faulio_api_base(url, &video_id, context)?;
        let api_base = api_base.trim_end_matches('/');
        let video_info = context
            .get_json(&format!("{api_base}/video/{video_id}"))
            .unwrap_or_else(|_| serde_json::json!({}));
        let player_info = context.get_json(&format!("{api_base}/video/{video_id}/player"))?;
        let headers = faulio_headers(url)?;
        let mut formats = Vec::new();
        let null = serde_json::Value::Null;
        let protocols = player_info
            .get("settings")
            .and_then(|settings| settings.get("protocols"))
            .unwrap_or(&null);
        faulio_add_manifest(protocols, "hls", "m3u8_native", &mut formats);
        faulio_add_manifest(protocols, "dash", "http_dash_segments", &mut formats);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Faulio video {video_id} has no playable HLS or DASH manifest"),
            ));
        }

        let empty = serde_json::Value::Null;
        let block = video_info
            .get("blocks")
            .and_then(serde_json::Value::as_array)
            .and_then(|blocks| blocks.first())
            .unwrap_or(&empty);
        let api_host = faulio_api_host(api_base, &video_id)?;
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(format!("{api_host}_{video_id}")));
        info.insert_if_some("display_id", json_string(block, "slug"));
        info.insert_if_some("title", json_string(block, "title"));
        info.insert_if_some("episode", json_string(block, "title"));
        info.insert_if_some("description", json_string(block, "description"));
        info.insert_if_some("series", json_string(block, "program_title"));
        info.insert_if_some("season_number", json_i64(block, "season_number"));
        info.insert_if_some("episode_number", json_i64(block, "episode"));
        info.insert_if_some("thumbnail", json_string(block, "image"));
        info.insert_if_some(
            "duration",
            block
                .get("duration")
                .and_then(|duration| json_i64(duration, "total")),
        );
        info.insert_if_some("age_limit", json_i64(block, "age_rating"));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("http_headers", headers);
        Ok(ExtractorResult::single(info))
    }
}

/// Native Faulio live-channel page/API/HLS-DASH extractor.
pub struct FaulioLiveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FaulioLiveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FaulioLiveExtractor {
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
        let channel_id = faulio_match_id(&self.matcher, url, "Faulio live")?;
        let api_base = faulio_api_base(url, &channel_id, context)?;
        let api_base = api_base.trim_end_matches('/');
        let channels = context
            .get_json(&format!("{api_base}/channels/{channel_id}"))
            .or_else(|_| context.get_json(&format!("{api_base}/channels")))?;
        let channel = faulio_find_channel(&channels, &channel_id).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Faulio live channel {channel_id} was not found"),
            )
        })?;
        let null = serde_json::Value::Null;
        let streams = channel.get("streams").unwrap_or(&null);
        let mut formats = Vec::new();
        faulio_add_manifest(streams, "hls", "m3u8_native", &mut formats);
        faulio_add_manifest(streams, "mpd", "http_dash_segments", &mut formats);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!(
                    "Faulio live channel {channel_id} has no playable HLS or DASH manifest"
                ),
            ));
        }
        let api_host = faulio_api_host(api_base, &channel_id)?;
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(format!("{api_host}_{channel_id}")));
        info.insert_if_some("title", json_string(channel, "title"));
        info.insert_if_some("description", json_string(channel, "description"));
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("http_headers", faulio_headers(url)?);
        info.insert("is_live", serde_json::json!(true));
        Ok(ExtractorResult::single(info))
    }
}

fn faulio_api_base(
    url: &str,
    video_id: &str,
    context: &ExtractionContext,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    let webpage = String::from_utf8_lossy(response.body());
    let config = json_object_after_marker(&webpage, "window.__NUXT__.config=").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Faulio page {video_id} has no Nuxt configuration"),
        )
    })?;
    let api_base = config
        .get("public")
        .and_then(|public| json_string(public, "TRANSLATIONS_API_URL"))
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(str::to_owned)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Faulio page {video_id} has no translations API URL"),
            )
        })?;
    Ok(api_base)
}

fn faulio_headers(page_url: &str) -> Result<serde_json::Value, ExtractorError> {
    let parsed = url::Url::parse(page_url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("invalid Faulio page URL: {error}"),
        )
    })?;
    let host = parsed.host_str().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            "Faulio page URL has no host",
        )
    })?;
    Ok(serde_json::json!({
        "Referer": page_url,
        "Origin": format!("{}://{host}", parsed.scheme()),
    }))
}

fn faulio_api_host(api_base: &str, video_id: &str) -> Result<String, ExtractorError> {
    url::Url::parse(api_base)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .filter(|host| !host.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Faulio API for {video_id} has no hostname"),
            )
        })
}

fn faulio_add_manifest(
    container: &serde_json::Value,
    key: &str,
    protocol: &str,
    formats: &mut Vec<serde_json::Value>,
) {
    let Some(media_url) = container
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
    else {
        return;
    };
    if formats.iter().any(|format| {
        format.get("url").and_then(serde_json::Value::as_str) == Some(media_url)
    }) {
        return;
    }
    formats.push(serde_json::json!({
        "url": media_url,
        "format_id": key,
        "protocol": protocol,
        "ext": "mp4",
    }));
}

fn faulio_find_channel<'a>(
    data: &'a serde_json::Value,
    channel_id: &str,
) -> Option<&'a serde_json::Value> {
    let channels = data.get("channels").unwrap_or(data);
    match channels {
        serde_json::Value::Array(values) => values
            .iter()
            .find(|channel| json_string(channel, "url") == Some(channel_id)),
        serde_json::Value::Object(values) => values
            .values()
            .find(|channel| json_string(channel, "url") == Some(channel_id)),
        _ => None,
    }
}

fn faulio_match_id(
    matcher: &Regex,
    url: &str,
    label: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("{label} URL has no ID"),
            )
        })
}
