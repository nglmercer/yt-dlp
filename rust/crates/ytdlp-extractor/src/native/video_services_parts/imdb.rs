/// Native IMDb trailer/video extractor.
pub struct ImdbExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ImdbExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ImdbExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "IMDb URL has no video ID")
            })?;
        let canonical_url = format!("https://www.imdb.com/video/vi{video_id}");
        let response = context.get(&canonical_url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let next_data = html_script_json(&webpage, "__NEXT_DATA__")?;
        let video_info = next_data
            .get("props")
            .and_then(|props| props.get("pageProps"))
            .and_then(|page_props| page_props.get("videoPlaybackData"))
            .and_then(|playback| playback.get("video"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("IMDb video {video_id} has no playback data"),
                )
            })?;
        let encodings = if let Some(playback_urls) = video_info
            .get("playbackURLs")
            .and_then(serde_json::Value::as_array)
            .filter(|values| !values.is_empty())
        {
            playback_urls.clone()
        } else {
            imdb_legacy_encodings(context, &video_id)?
        };
        let mut formats = Vec::new();
        for encoding in &encodings {
            let Some(media_url) = json_string(encoding, "url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let url_extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
            let extension = imdb_mimetype_extension(json_string(encoding, "mimeType"))
                .unwrap_or_else(|| url_extension.clone());
            let is_hls = extension == "m3u8" || url_extension == "m3u8";
            let mut format = serde_json::json!({
                "url": media_url,
                "ext": if is_hls { "mp4" } else { extension.as_str() },
                "quality": imdb_quality(encoding),
            });
            if is_hls {
                format["protocol"] = serde_json::json!("m3u8_native");
                format["format_id"] = serde_json::json!("hls");
            } else if let Some(format_id) = imdb_format_id(encoding) {
                format["format_id"] = serde_json::json!(format_id);
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("IMDb video {video_id} has no playable encodings"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let title = imdb_nested_string(video_info, &[&["name", "value"], &["primaryTitle", "titleText", "text"]])
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .or_else(|| html_title_value(&webpage))
            .unwrap_or_else(|| video_id.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", first.get("url").cloned().unwrap_or(serde_json::Value::Null));
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some(
            "alt_title",
            json_string(&next_data, "videoSubTitle"),
        );
        info.insert_if_some(
            "description",
            imdb_nested_string(video_info, &[&["description", "value"]]),
        );
        info.insert_if_some(
            "thumbnail",
            imdb_nested_string(video_info, &[&["thumbnail", "url"]]),
        );
        info.insert_if_some(
            "duration",
            video_info
                .get("runtime")
                .and_then(|runtime| json_f64(runtime, "value")),
        );
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn imdb_legacy_encodings(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let payload = serde_json::json!({
        "type": "VIDEO_PLAYER",
        "subType": "FORCE_LEGACY",
        "id": format!("vi{video_id}"),
    });
    let encoded = imdb_base64_encode(
        &serde_json::to_vec(&payload).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("could not encode IMDb legacy request: {error}"),
            )
        })?,
    );
    let mut api_url = url::Url::parse("https://www.imdb.com/ve/data/VIDEO_PLAYBACK_DATA")
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid IMDb legacy API URL: {error}"),
            )
        })?;
    api_url.query_pairs_mut().append_pair("key", &encoded);
    let response = context.get(api_url.as_str())?;
    let data = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid IMDb legacy JSON for {video_id}: {error}"),
        )
    })?;
    Ok(data
        .as_array()
        .and_then(|values| values.first())
        .and_then(|value| value.get("videoLegacyEncodings"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn imdb_nested_string(value: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        path.iter()
            .try_fold(value, |value, key| value.get(*key))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    })
}

fn imdb_format_id(encoding: &serde_json::Value) -> Option<String> {
    imdb_nested_string(encoding, &[&["displayName", "value"], &["definition"]])
}

fn imdb_quality(encoding: &serde_json::Value) -> i64 {
    match imdb_format_id(encoding)
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "1080p" => 3,
        "720p" => 2,
        "480p" => 1,
        "sd" => 0,
        _ => -1,
    }
}

fn imdb_mimetype_extension(value: Option<&str>) -> Option<String> {
    match value?.split(';').next()?.trim().to_ascii_lowercase().as_str() {
        "video/mp4" => Some("mp4".to_owned()),
        "video/quicktime" => Some("mov".to_owned()),
        "video/x-flv" => Some("flv".to_owned()),
        "application/x-mpegurl" | "application/vnd.apple.mpegurl" => Some("m3u8".to_owned()),
        _ => None,
    }
}

fn imdb_base64_encode(value: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(value.len().div_ceil(3) * 4);
    for chunk in value.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        encoded.push(TABLE[(first >> 2) as usize] as char);
        encoded.push(TABLE[((first & 0x03) << 4 | second >> 4) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[((second & 0x0f) << 2 | third >> 6) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(third & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}
