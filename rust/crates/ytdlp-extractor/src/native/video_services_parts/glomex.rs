/// Native Glomex API-backed video and playlist extractor.
pub struct GlomexEmbedExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GlomexEmbedExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GlomexEmbedExtractor {
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
        let playlist_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Glomex embed URL has no playlist ID",
                )
            })?;
        let integration = url_query_value(url, "integrationId").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Glomex embed URL has no integration ID",
            )
        })?;
        let current_url = url_query_value(url, "origin")
            .unwrap_or_else(|| "https://player.glomex.com/".to_owned());
        let api_url = glomex_api_url(&playlist_id, &integration, &current_url);
        let data = context.get_json(&api_url)?;
        let videos = data.get("videos").and_then(serde_json::Value::as_array).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Glomex response for {playlist_id} has no videos array"),
            )
        })?;
        if videos.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("no videos found for Glomex playlist {playlist_id}"),
            ));
        }
        let entries = videos
            .iter()
            .map(|video| glomex_video_info(video, &playlist_id))
            .collect::<Result<Vec<_>, _>>()?;
        if entries.len() == 1 {
            return Ok(ExtractorResult::Single(
                entries.into_iter().next().ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        "Glomex response lost its only video entry",
                    )
                })?,
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Glomex page wrapper. It builds the player URL used by the API
/// extractor, so the original page never needs a Python-side delegation.
pub struct GlomexExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GlomexExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GlomexExtractor {
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
        _context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Glomex URL has no video ID")
            })?;
        let player_url = glomex_player_url(&video_id, "19syy24xjn1oqlpc", Some(url));
        Ok(ExtractorResult::Redirect {
            url: player_url,
            ie_key: Some("GlomexEmbed".to_owned()),
        })
    }
}

fn glomex_api_url(video_id: &str, integration: &str, current_url: &str) -> String {
    let mut api_url = url::Url::parse(
        "https://integration-cloudfront-eu-west-1.mes.glomex.cloud/",
    )
    .expect("static Glomex API URL");
    api_url
        .query_pairs_mut()
        .append_pair("integration_id", integration)
        .append_pair("playlist_id", video_id)
        .append_pair("current_url", current_url);
    api_url.to_string()
}

fn glomex_player_url(video_id: &str, integration: &str, origin: Option<&str>) -> String {
    let mut player_url = url::Url::parse(
        "https://player.glomex.com/integration/1/iframe-player.html",
    )
    .expect("static Glomex player URL");
    let mut query = player_url.query_pairs_mut();
    query
        .append_pair("playlistId", video_id)
        .append_pair("integrationId", integration);
    if let Some(origin) = origin {
        query.append_pair("origin", origin);
    }
    drop(query);
    player_url.to_string()
}

fn glomex_video_info(
    video: &serde_json::Value,
    playlist_id: &str,
) -> Result<InfoDict, ExtractorError> {
    if video.get("error_code").and_then(serde_json::Value::as_str)
        == Some("contentGeoblocked")
    {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Glomex video {playlist_id} is geo-blocked and requires native geo handling"
            ),
        ));
    }
    let source = video
        .get("source")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Glomex video {playlist_id} has no source formats"),
            )
        })?;
    let mut formats = Vec::new();
    for (format_id, value) in source {
        let Some(media_url) = value.as_str().filter(|value| {
            value.starts_with("http://") || value.starts_with("https://")
        }) else {
            continue;
        };
        let source_ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let mut format = serde_json::json!({
            "url": media_url,
            "format_id": format_id,
            "ext": source_ext,
        });
        if source_ext == "m3u8" {
            format["protocol"] = serde_json::json!("m3u8_native");
            format["ext"] = serde_json::json!("mp4");
        }
        formats.push(format);
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Glomex video {playlist_id} has no playable source formats"),
        ));
    }
    let video_id = json_string(video, "clip_id")
        .or_else(|| json_string(video, "id"))
        .unwrap_or(playlist_id)
        .to_owned();
    let first_url = formats
        .first()
        .and_then(|format| format.get("url"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Glomex video {video_id} has no first format URL"),
            )
        })?;
    let first_ext = formats
        .first()
        .and_then(|format| format.get("ext"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("mp4");
    let thumbnails = glomex_thumbnails(video);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert_if_some("title", json_string(video, "title"));
    info.insert_if_some("description", json_string(video, "description"));
    info.insert_if_some("duration", json_i64(video, "clip_duration"));
    info.insert_if_some(
        "timestamp",
        glomex_timestamp(video.get("created_at")),
    );
    if let Some(thumbnail) = thumbnails.first() {
        info.insert_if_some(
            "thumbnail",
            thumbnail.get("url").and_then(serde_json::Value::as_str),
        );
    }
    info.insert("url", serde_json::json!(first_url));
    info.insert("ext", serde_json::json!(first_ext));
    info.insert("formats", serde_json::Value::Array(formats));
    info.insert("thumbnails", serde_json::Value::Array(thumbnails));
    info.insert("subtitles", serde_json::json!({}));
    Ok(info)
}

fn glomex_thumbnails(video: &serde_json::Value) -> Vec<serde_json::Value> {
    video
        .get("images")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .chain(video.get("image"))
        .filter_map(|image| {
            let image_url = image.get("url").and_then(serde_json::Value::as_str)?;
            Some(serde_json::json!({
                "id": image.get("id").and_then(serde_json::Value::as_str),
                "url": format!("{image_url}/profile:player-960x540"),
                "width": 960,
                "height": 540,
            }))
        })
        .collect()
}

fn glomex_timestamp(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
            .or_else(|| value.as_str().and_then(|value| parse_timestamp(value.to_owned())))
    })
}
