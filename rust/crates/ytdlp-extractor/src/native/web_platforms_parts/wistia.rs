/// Native Wistia media extractor backed by the embed JSON endpoint.
pub struct WistiaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WistiaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WistiaExtractor {
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
                "Wistia URL did not match its native pattern",
            )
        })?;
        let media_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Wistia URL has no ID")
            })?;
        let config = wistia_embed_config(context, "medias", media_id, url)?;
        Ok(ExtractorResult::single(wistia_media_info(&config)?))
    }
}

/// Native Wistia playlist extractor. Wistia playlist responses embed media
/// configs, which are materialized so the Rust CLI can select entries without
/// lazy Python callbacks.
pub struct WistiaPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WistiaPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WistiaPlaylistExtractor {
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
                "Wistia playlist URL did not match its native pattern",
            )
        })?;
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Wistia playlist URL has no ID",
                )
            })?;
        let config = wistia_embed_config(context, "playlists", playlist_id, url)?;
        let media_values = config
            .as_array()
            .and_then(|values| values.first())
            .and_then(|value| value.get("medias"))
            .and_then(serde_json::Value::as_array)
            .or_else(|| config.get("medias").and_then(serde_json::Value::as_array))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Wistia playlist {playlist_id} has no media list"),
                )
            })?;
        let mut entries = Vec::new();
        for media in media_values {
            let Some(embed_config) = media.get("embed_config") else {
                continue;
            };
            entries.push(wistia_media_info(embed_config)?);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Wistia playlist {playlist_id} has no playable media"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Wistia channel extractor. API-backed channels are materialized into
/// media InfoDicts; webpage JSONP fallback and password prompts are explicit
/// TODOs because they need additional native configuration inputs.
pub struct WistiaChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WistiaChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WistiaChannelExtractor {
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
                "Wistia channel URL did not match its native pattern",
            )
        })?;
        let channel_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Wistia channel URL has no ID",
                )
            })?;
        if let Ok(parsed) = url::Url::parse(url) {
            if let Some(media_id) = parsed
                .query_pairs()
                .find_map(|(key, value)| {
                    matches!(key.as_ref(), "wmediaid" | "wvideoid" | "wvideo")
                        .then(|| value.into_owned())
                })
                .filter(|media_id| !media_id.is_empty())
            {
                let extractor = WistiaExtractor::new(ExtractorDescriptor::new(
                    "WistiaIE",
                    "Wistia",
                    r"(?:wistia:|https?://(?:\w+\.)?wistia\.(?:net|com)/(?:embed/)?(?:iframe|medias)/)(?P<id>[a-z0-9]{10})",
                    true,
                ))?;
                return extractor.extract_with_context(&format!("wistia:{media_id}"), context);
            }
        }
        let config = wistia_embed_config(context, "channel", channel_id, url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Wistia channel webpage JSONP fallback is not ported ({error})"),
            )
        })?;
        let series = config
            .get("series")
            .and_then(serde_json::Value::as_array)
            .and_then(|series| series.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Wistia channel {channel_id} has no series"),
                )
            })?;
        let mut media_ids: Vec<String> = Vec::new();
        if let Some(sections) = series.get("sections").and_then(serde_json::Value::as_array) {
            for section in sections {
                for key in ["videos", "episodes"] {
                    let Some(values) = section.get(key).and_then(serde_json::Value::as_array)
                    else {
                        continue;
                    };
                    for value in values {
                        let Some(media_id) = json_string(value, "hashedId") else {
                            continue;
                        };
                        if !media_ids.iter().any(|existing| existing == media_id) {
                            media_ids.push(media_id.to_owned());
                        }
                    }
                }
            }
        }
        let mut entries = Vec::new();
        for media_id in media_ids {
            let media_config = wistia_embed_config(context, "medias", &media_id, url)?;
            entries.push(wistia_media_info(&media_config)?);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Wistia channel {channel_id} has no playable media"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        info.insert_if_some("title", json_string(series, "title"));
        info.insert_if_some("description", json_string(series, "description"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn wistia_embed_config(
    context: &ExtractionContext,
    config_type: &str,
    config_id: &str,
    referer: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let config = native_get_json_with_headers(
        context,
        &format!("https://fast.wistia.net/embed/{config_type}/{config_id}.json"),
        &[("Referer", referer)],
    )?;
    if let Some(error) = json_string(&config, "error") {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Wistia {config_type} {config_id}: {error}"),
        ));
    }
    if config
        .get("media")
        .and_then(|media| {
            media
                .get("embed_options")
                .or_else(|| media.get("embedOptions"))
        })
        .and_then(|options| options.get("plugin"))
        .and_then(|plugin| plugin.get("passwordProtectedVideo"))
        .and_then(|password| password.get("on"))
        .and_then(serde_json::Value::as_str)
        == Some("true")
    {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            "TODO: Wistia password-protected media requires a native video-password option",
        ));
    }
    Ok(config)
}

fn wistia_media_info(config: &serde_json::Value) -> Result<InfoDict, ExtractorError> {
    let media = config.get("media").unwrap_or(config);
    let media_id = json_string(media, "hashedId").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Wistia media config has no hashedId",
        )
    })?;
    let title = json_string(media, "name").unwrap_or(media_id);
    let mut formats = Vec::new();
    let mut thumbnails = Vec::new();
    if let Some(assets) = media.get("assets").and_then(serde_json::Value::as_array) {
        for asset in assets {
            let Some(raw_url) = json_string(asset, "url") else {
                continue;
            };
            if json_i64(asset, "status").is_some_and(|status| status != 2) {
                continue;
            }
            let asset_type = json_string(asset, "type").unwrap_or("unknown");
            if matches!(asset_type, "preview" | "storyboard") {
                continue;
            }
            if matches!(asset_type, "still" | "still_image") {
                thumbnails.push(serde_json::json!({
                    "url": wistia_replace_bin_extension(raw_url, asset),
                    "width": json_i64(asset, "width"),
                    "height": json_i64(asset, "height"),
                    "filesize": json_i64(asset, "size"),
                }));
                continue;
            }
            let display_name = json_string(asset, "display_name");
            let format_id = if asset_type.ends_with("_video") {
                display_name
                    .map(|display_name| {
                        format!("{}-{display_name}", asset_type.trim_end_matches("_video"))
                    })
                    .unwrap_or_else(|| asset_type.to_owned())
            } else {
                asset_type.to_owned()
            };
            let container = json_string(asset, "container");
            let asset_ext = json_string(asset, "ext")
                .map(str::to_owned)
                .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(raw_url), "bin"));
            let is_hls = container == Some("m3u8") || asset_ext == "m3u8";
            let mut format = serde_json::json!({
                "format_id": format_id,
                "url": raw_url,
                "quality": if asset_type == "original" { 1 } else { 0 },
                "tbr": json_i64(asset, "bitrate"),
            });
            if display_name == Some("Audio") {
                format["vcodec"] = serde_json::json!("none");
            } else {
                format["width"] = json_i64(asset, "width")
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value));
                format["height"] = json_i64(asset, "height")
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value));
                format["vcodec"] = json_string(asset, "codec")
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value));
            }
            if is_hls {
                let mut hls = format.clone();
                hls["ext"] = serde_json::json!("mp4");
                hls["protocol"] = serde_json::json!("m3u8_native");
                formats.push(hls);
                let mut ts = format;
                ts["format_id"] = serde_json::json!(format_id.replace("hls-", "ts-"));
                ts["url"] = serde_json::json!(raw_url.replace(".bin", ".ts"));
                ts["ext"] = serde_json::json!("ts");
                formats.push(ts);
            } else {
                format["container"] =
                    container.map_or(serde_json::Value::Null, |value| serde_json::json!(value));
                format["ext"] = serde_json::json!(asset_ext);
                format["filesize"] = json_i64(asset, "size")
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value));
                formats.push(format);
            }
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Wistia media {media_id} has no playable assets"),
        ));
    }
    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(media_id));
    info.insert("title", serde_json::json!(title));
    info.insert(
        "url",
        first.get("url").cloned().unwrap_or(serde_json::Value::Null),
    );
    info.insert(
        "ext",
        first
            .get("ext")
            .cloned()
            .unwrap_or_else(|| serde_json::json!("mp4")),
    );
    info.insert("formats", serde_json::Value::Array(formats));
    info.insert_if_some("description", json_string(media, "seoDescription"));
    info.insert_if_some("duration", json_f64(media, "duration"));
    info.insert_if_some("timestamp", json_i64(media, "createdAt"));
    if !thumbnails.is_empty() {
        info.insert("thumbnails", serde_json::Value::Array(thumbnails));
    }
    if let Some(captions) = media.get("captions").and_then(serde_json::Value::as_array) {
        let mut subtitles = serde_json::Map::new();
        for caption in captions {
            let Some(language) = json_string(caption, "language") else {
                continue;
            };
            subtitles.insert(
                language.to_owned(),
                serde_json::json!([{
                    "url": format!("https://fast.wistia.net/embed/captions/{media_id}.vtt?language={language}")
                }]),
            );
        }
        if !subtitles.is_empty() {
            info.insert("subtitles", serde_json::Value::Object(subtitles));
        }
    }
    Ok(info)
}

fn wistia_replace_bin_extension(url: &str, asset: &serde_json::Value) -> String {
    let extension = json_string(asset, "ext")
        .map(str::to_owned)
        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(url), "jpg"));
    url.replace(".bin", &format!(".{extension}"))
}
