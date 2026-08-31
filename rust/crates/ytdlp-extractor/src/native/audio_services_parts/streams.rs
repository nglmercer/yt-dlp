/// Native Yandex Disk extractor. The page store, public download URL, and
/// server-provided video streams are consumed directly by Rust.
pub struct YandexDiskExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl YandexDiskExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for YandexDiskExtractor {
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
                "Yandex Disk URL did not match its native pattern",
            )
        })?;
        let mut video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Yandex Disk URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let store = html_script_json(&html, "store-prefetch")?;
        let resource_id = json_value_string(store.get("rootResourceId")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Yandex Disk store has no root resource ID",
            )
        })?;
        let resource = store
            .get("resources")
            .and_then(serde_json::Value::as_object)
            .and_then(|resources| resources.get(&resource_id))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Yandex Disk store has no root resource",
                )
            })?;
        let title = json_string(resource, "name")
            .map(str::to_owned)
            .unwrap_or_else(|| video_id.clone());
        if let Some(public_key) = resource
            .get("meta")
            .and_then(|meta| json_string(meta, "short_url"))
        {
            if let Some(public_id) = self
                .matcher
                .captures(public_key)
                .ok()
                .flatten()
                .and_then(|captures| captures.name("id"))
                .map(|value| value.as_str().to_owned())
            {
                video_id = public_id;
            }
        }
        let meta = resource.get("meta").unwrap_or(&serde_json::Value::Null);
        let mut formats = Vec::new();
        let mut source_request =
            Request::new("https://cloud-api.yandex.net/v1/disk/public/resources/download");
        source_request.update_query(&[("public_key".to_owned(), url.to_owned())]);
        if let Ok(source) = context.request(&source_request) {
            if let Ok(source_json) = serde_json::from_slice::<serde_json::Value>(source.body()) {
                if let Some(source_url) = json_string(&source_json, "href") {
                    let ext = yt_dlp_core::determine_ext(
                        Some(&title),
                        json_string(meta, "ext")
                            .or_else(|| json_string(meta, "mime_type"))
                            .unwrap_or("mp4"),
                    );
                    formats.push(serde_json::json!({
                        "url": source_url,
                        "format_id": "source",
                        "ext": ext,
                        "quality": 1,
                        "filesize": json_i64(meta, "size"),
                    }));
                }
            }
        }
        if let Some(video_streams) = resource.get("videoStreams") {
            if let Some(videos) = video_streams
                .get("videos")
                .and_then(serde_json::Value::as_array)
            {
                for video in videos {
                    let Some(stream_url) = json_string(video, "url") else {
                        continue;
                    };
                    let size = video.get("size");
                    let height = json_i64(size.unwrap_or(&serde_json::Value::Null), "height");
                    let format_id =
                        height.map_or_else(|| "hls".to_owned(), |height| format!("hls-{height}p"));
                    formats.push(serde_json::json!({
                        "url": stream_url,
                        "format_id": format_id,
                        "ext": "mp4",
                        "height": height,
                        "width": json_i64(size.unwrap_or(&serde_json::Value::Null), "width"),
                        "protocol": "m3u8_native",
                    }));
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Yandex Disk resource {video_id} has no native media formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
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
        if let Some(duration) = video_streams_duration(resource) {
            info.insert("duration", serde_json::json!(duration));
        }
        if let Some(uid) = json_string(resource, "uid") {
            info.insert("uploader_id", serde_json::json!(uid));
            if let Some(display_name) = store
                .get("users")
                .and_then(serde_json::Value::as_object)
                .and_then(|users| users.get(uid))
                .and_then(|user| json_string(user, "displayName"))
            {
                info.insert("uploader", serde_json::json!(display_name));
            }
        }
        info.insert_if_some("view_count", json_i64(meta, "views_counter"));
        Ok(ExtractorResult::single(info))
    }
}

fn video_streams_duration(resource: &serde_json::Value) -> Option<f64> {
    resource
        .get("videoStreams")
        .and_then(|streams| json_f64(streams, "duration"))
        .map(|duration| duration / 1000.0)
}

/// Native Rumble embed API extractor. The embed JSON exposes direct, audio,
/// HLS, captions, live state, and author metadata without executing its player.
pub struct RumbleEmbedExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RumbleEmbedExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RumbleEmbedExtractor {
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
                "Rumble embed URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Rumble embed URL has no ID")
            })?;
        let mut request = Request::new("https://rumble.com/embedJS/u3/");
        request.update_query(&[
            ("request".to_owned(), "video".to_owned()),
            ("ver".to_owned(), "2".to_owned()),
            ("v".to_owned(), video_id.to_owned()),
        ]);
        let video = context.get_json(request.url())?;
        let live_status = match (
            json_i64(&video, "live"),
            json_bool(&video, "livestream_has_dvr"),
        ) {
            (Some(0), Some(true)) => "was_live",
            (Some(0), _) => "not_live",
            (Some(1), Some(false)) => "was_live",
            (Some(1), _) => "is_upcoming",
            (Some(2), _) => "is_live",
            _ => "",
        };
        let mut formats = Vec::new();
        if let Some(format_groups) = video.get("ua").and_then(serde_json::Value::as_object) {
            for (format_type, format_info) in format_groups {
                let candidates = match format_info {
                    serde_json::Value::Array(values) => {
                        values.iter().map(|value| (None, value)).collect::<Vec<_>>()
                    }
                    serde_json::Value::Object(values) => values
                        .iter()
                        .map(|(height, value)| (Some(height.as_str()), value))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                for (height_hint, video_info) in candidates {
                    let Some(media_url) = json_string(video_info, "url") else {
                        continue;
                    };
                    if format_type == "tar" {
                        continue;
                    }
                    let meta = video_info.get("meta").unwrap_or(&serde_json::Value::Null);
                    let height = json_i64(meta, "h")
                        .or_else(|| height_hint.and_then(|height| height.parse::<i64>().ok()));
                    if format_type == "hls" {
                        formats.push(serde_json::json!({
                            "url": media_url,
                            "format_id": "hls",
                            "ext": "mp4",
                            "protocol": "m3u8_native",
                        }));
                        continue;
                    }
                    let is_timeline = format_type == "timeline";
                    let is_audio = format_type == "audio";
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": height.map_or_else(
                            || format_type.to_owned(),
                            |height| format!("{format_type}-{height}p")
                        ),
                        "format_note": if is_timeline { "Timeline" } else { "" },
                        "vcodec": if is_audio { "none" } else { "unknown" },
                        "acodec": if is_timeline { "none" } else { "unknown" },
                        "fps": if is_timeline || is_audio {
                            serde_json::Value::Null
                        } else {
                            video.get("fps").cloned().unwrap_or(serde_json::Value::Null)
                        },
                    });
                    for (source, target) in [
                        ("bitrate", "tbr"),
                        ("size", "filesize"),
                        ("w", "width"),
                        ("h", "height"),
                    ] {
                        if let Some(value) = meta.get(source) {
                            format[target] = value.clone();
                        }
                    }
                    formats.push(format);
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Rumble video {video_id} has no playable formats"),
            ));
        }
        let author = video.get("author").unwrap_or(&serde_json::Value::Null);
        let mut info = InfoDict::new();
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            json_string(&video, "title").map(unescape_html_attribute),
        );
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
        info.insert_if_some(
            "timestamp",
            json_string(&video, "pubDate").and_then(yt_dlp_core::parse_iso8601),
        );
        info.insert_if_some("channel", json_string(author, "name"));
        info.insert_if_some("channel_url", json_string(author, "url"));
        info.insert_if_some("uploader", json_string(author, "name"));
        if !live_status.is_empty() {
            info.insert("live_status", serde_json::json!(live_status));
        }
        if live_status != "is_live" && live_status != "post_live" {
            info.insert_if_some("duration", json_i64(&video, "duration"));
        }
        let mut thumbnails = Vec::new();
        if let Some(values) = video.get("t").and_then(serde_json::Value::as_array) {
            thumbnails.extend(values.iter().filter_map(|thumbnail| {
                let url = json_string(thumbnail, "i")?;
                Some(serde_json::json!({
                    "url": url,
                    "width": json_i64(thumbnail, "w"),
                    "height": json_i64(thumbnail, "h"),
                }))
            }));
        }
        if thumbnails.is_empty() {
            if let Some(thumbnail) = json_string(&video, "i") {
                thumbnails.push(serde_json::json!({"url": thumbnail}));
            }
        }
        if !thumbnails.is_empty() {
            info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        }
        if let Some(captions) = video.get("cc").and_then(serde_json::Value::as_object) {
            let subtitles = captions
                .iter()
                .filter_map(|(language, caption)| {
                    let path = json_string(caption, "path")?;
                    Some((
                        language.clone(),
                        serde_json::json!([{
                            "url": path,
                            "name": json_string(caption, "language").unwrap_or("")
                        }]),
                    ))
                })
                .collect::<serde_json::Map<_, _>>();
            if !subtitles.is_empty() {
                info.insert("subtitles", serde_json::Value::Object(subtitles));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Clyp API extractor. The API response already contains stable media
/// URLs, so this port does not depend on browser JavaScript or an embedded
/// interpreter.
pub struct ClypExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ClypExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ClypExtractor {
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
        let audio_id = last_path_segment(url)?;
        let mut api_request = Request::new(format!("https://api.clyp.it/{audio_id}"));
        if let Ok(parsed) = url::Url::parse(url) {
            if let Some(token) = parsed
                .query_pairs()
                .find(|(name, _)| name == "token")
                .map(|(_, value)| value.into_owned())
            {
                api_request.update_query(&[("token".to_owned(), token)]);
            }
        }
        let metadata = context.get_json(api_request.url())?;
        let mut formats = Vec::new();
        for secure in ["", "Secure"] {
            for extension in ["Ogg", "Mp3"] {
                let key = format!("{secure}{extension}Url");
                let Some(format_url) = json_string(&metadata, &key) else {
                    continue;
                };
                formats.push(serde_json::json!({
                    "url": format_url,
                    "format_id": format!("{secure}{extension}"),
                    "ext": extension.to_ascii_lowercase(),
                    "vcodec": "none",
                    "acodec": extension.to_ascii_lowercase(),
                }));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Clyp API returned no playable formats for {audio_id}"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert(
            "title",
            serde_json::json!(json_string(&metadata, "Title").unwrap_or(&audio_id)),
        );
        info.insert_if_some("description", json_string(&metadata, "Description"));
        info.insert_if_some("duration", json_f64(&metadata, "Duration"));
        info.insert("formats", serde_json::Value::Array(formats));
        if let Some(value) = first.get("url") {
            info.insert("url", value.clone());
        }
        if let Some(value) = first.get("ext") {
            info.insert("ext", value.clone());
        }
        Ok(ExtractorResult::single(info))
    }
}
