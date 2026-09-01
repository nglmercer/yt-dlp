/// Native Dacast playback API and playlist extractors.
pub struct DacastVodExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct DacastPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DacastVodExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl DacastPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DacastVodExtractor {
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
        let (user_id, video_id) = dacast_url_parts(&self.matcher, url, "Dacast VOD")?;
        let query = dacast_query(&user_id, "vod", &video_id, url);
        let info = dacast_api_json(
            context,
            "https://playback.dacast.com/content/info",
            &query,
            &[],
        )
        .unwrap_or_else(|_| serde_json::json!({}));
        let access = dacast_api_json(
            context,
            "https://playback.dacast.com/content/access",
            &query,
            &[403],
        )?;

        if let Some(error) = json_string(&access, "error") {
            if matches!(error, "Broadcaster has been blocked" | "Content is offline") {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Dacast content {video_id}: {error}"),
                ));
            }
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Dacast API says \"{error}\""),
            ));
        }

        let hls_url = json_string(&access, "hls")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Dacast content {video_id} has no HLS URL"),
                )
            })?;
        let lower_hls_url = hls_url.to_ascii_lowercase();
        if lower_hls_url.contains("drm_ext") || lower_hls_url.contains("/uspaes/") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Dacast native extractor does not implement DRM/AES HLS playback: {hls_url}"
                ),
            ));
        }

        let formats = serde_json::json!([{
            "url": hls_url,
            "format_id": "hls",
            "protocol": "m3u8_native",
            "ext": "mp4",
        }]);
        let content_info = info.get("contentInfo").unwrap_or(&serde_json::Value::Null);
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(video_id));
        output.insert("uploader_id", serde_json::json!(user_id));
        output.insert_if_some("title", json_string(content_info, "title"));
        output.insert_if_some("duration", json_f64(content_info, "duration"));
        output.insert_if_some(
            "thumbnail",
            json_string(content_info, "thumbnailUrl").map(str::to_owned),
        );
        output.insert("url", serde_json::json!(hls_url));
        output.insert("ext", serde_json::json!("mp4"));
        output.insert("formats", formats);
        output.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(output))
    }
}

impl InfoExtractor for DacastPlaylistExtractor {
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
        let (user_id, playlist_id) = dacast_url_parts(&self.matcher, url, "Dacast playlist")?;
        let query = dacast_query(&user_id, "playlist", &playlist_id, url);
        let response = dacast_api_json(
            context,
            "https://playback.dacast.com/content/info",
            &query,
            &[],
        )?;
        let content_info = response
            .get("contentInfo")
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Dacast playlist {playlist_id} has no content information"),
                )
            })?;
        let contents = content_info
            .get("features")
            .and_then(|features| features.get("playlist"))
            .and_then(|playlist| playlist.get("contents"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Dacast playlist {playlist_id} has no video entries"),
                )
            })?;

        let mut entries = Vec::new();
        for content in contents {
            let Some(content_id) = json_string(content, "id") else {
                continue;
            };
            let Some((entry_user_id, entry_video_id)) = content_id.split_once("-vod-") else {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: Dacast playlist contains an unsupported content ID: {content_id}"
                    ),
                ));
            };
            let target_url = format!(
                "https://iframe.dacast.com/vod/{entry_user_id}/{entry_video_id}"
            );
            let mut entry = native_url_result(&target_url);
            entry.insert("ie_key", serde_json::json!("DacastVOD"));
            entry.insert_if_some("title", json_string(content, "title"));
            entries.push(entry);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Dacast playlist {playlist_id} has no video entries"),
            ));
        }

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", json_string(content_info, "title"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn dacast_url_parts(
    matcher: &Regex,
    url: &str,
    kind: &str,
) -> Result<(String, String), ExtractorError> {
    let captures = matcher.captures(url).ok().flatten().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("{kind} URL did not match its native pattern"),
        )
    })?;
    let user_id = captures
        .name("user_id")
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, format!("{kind} URL has no user ID"))
        })?;
    let media_id = captures
        .name("id")
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, format!("{kind} URL has no media ID"))
        })?;
    Ok((user_id, media_id))
}

fn dacast_query(user_id: &str, content_type: &str, content_id: &str, source_url: &str) -> Vec<(String, String)> {
    let mut query = vec![
        (
            "contentId".to_owned(),
            format!("{user_id}-{content_type}-{content_id}"),
        ),
        ("provider".to_owned(), "universe".to_owned()),
    ];
    if let Some(uss_token) = url_query_value(source_url, "uss_token") {
        query.push(("uss_token".to_owned(), uss_token));
    }
    query
}

fn dacast_api_json(
    context: &ExtractionContext,
    endpoint: &str,
    query: &[(String, String)],
    accepted_statuses: &[u16],
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(endpoint);
    request.update_query(query);
    let response = context.request_with_status(&request, accepted_statuses)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Dacast API JSON from {}: {error}", response.url()),
        )
    })
}
