/// Native AtScale conference event playlist extractor. Event pages expose
/// canonical video URLs in data-url attributes; each entry is expanded by
/// the native Generic extractor so OpenGraph/HTML5 media is preserved.
pub struct AtScaleConfEventExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AtScaleConfEventExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AtScaleConfEventExtractor {
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
                    "AtScale event URL did not contain a playlist ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let link_matcher =
            Regex::new(r#"(?is)\bdata-url\s*=\s*"((?:https?://)(?:www\.)?atscaleconference\.com/videos/[^"]+)""#)
                .map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("invalid AtScale video link matcher: {error}"),
                    )
                })?;
        let generic =
            GenericExtractor::new(ExtractorDescriptor::new("GenericIE", "Generic", "", true));
        let mut entries = Vec::new();
        for captures in link_matcher.captures_iter(&html).flatten() {
            let Some(entry_url) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let entry = generic.extract_with_context(entry_url, context)?;
            match entry {
                ExtractorResult::Single(info) => entries.push(info),
                ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                    return Err(ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        "AtScale video entry did not resolve to a single native result",
                    ));
                }
            }
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("AtScale event {playlist_id} has no video entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", html_meta_value(&html, "og:title"));
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native NZZ article/video playlist extractor. NZZ embeds one or more
/// JWPlayer settings objects in page scripts; these are parsed as data and
/// never evaluated as JavaScript.
pub struct NzzExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NzzExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NzzExtractor {
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
        let page_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "NZZ URL did not contain a page ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let script_matcher = Regex::new(
            r#"(?is)<script\b[^>]*\bdata-hid\s*=\s*"jw-video-jw[^"]*"[^>]*>(.*?)</script>"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid NZZ JWPlayer script matcher: {error}"),
            )
        })?;
        let mut entries = Vec::new();
        for captures in script_matcher.captures_iter(&html).flatten() {
            let Some(script) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let Some(settings) = json_object_after_marker(script, "var settings") else {
                continue;
            };
            let items = settings
                .get("playlist")
                .and_then(serde_json::Value::as_array)
                .map(|items| items.iter().collect::<Vec<_>>())
                .unwrap_or_else(|| vec![&settings]);
            for item in items {
                if let Some(entry) = nzz_jw_entry(item, &page_id) {
                    entries.push(entry);
                }
            }
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("NZZ page {page_id} has no playable JWPlayer entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(page_id));
        info.insert_if_some("title", html_meta_value(&html, "og:title"));
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn nzz_jw_entry(item: &serde_json::Value, fallback_id: &str) -> Option<InfoDict> {
    let sources = item
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .map(|sources| sources.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| vec![item]);
    let mut formats = Vec::new();
    for (index, source) in sources.into_iter().enumerate() {
        let raw_url = json_string(source, "file")
            .or_else(|| json_string(source, "url"))
            .filter(|value| !value.is_empty())?;
        if raw_url.starts_with("rtmp:") {
            continue;
        }
        let source_type = json_string(source, "type").unwrap_or("");
        let source_ext = source_type
            .split(';')
            .next()
            .and_then(|value| mimetype_extension(Some(value)))
            .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(raw_url), "mp4"));
        let source_ext = yt_dlp_core::determine_ext(Some(raw_url), &source_ext);
        let is_hls = source_type.eq_ignore_ascii_case("hls") || source_ext == "m3u8";
        let is_dash = source_type.eq_ignore_ascii_case("dash") || source_ext == "mpd";
        let mut format = serde_json::json!({
            "url": proto_relative_url(raw_url, "https:"),
            "format_id": json_string(source, "label")
                .map(str::to_owned)
                .unwrap_or_else(|| format!("http-{index}")),
            "ext": if is_hls || is_dash { "mp4" } else { source_ext.as_str() },
        });
        if is_hls {
            format["protocol"] = serde_json::json!("m3u8_native");
        } else if is_dash {
            format["protocol"] = serde_json::json!("http_dash_segments");
        }
        if let Some(value) = json_i64(source, "width") {
            format["width"] = serde_json::json!(value);
        }
        if let Some(value) = json_i64(source, "height") {
            format["height"] = serde_json::json!(value);
        }
        if let Some(value) = json_i64(source, "bitrate") {
            format["tbr"] = serde_json::json!(value as f64 / 1000.0);
        }
        formats.push(format);
    }
    if formats.is_empty() {
        return None;
    }
    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let id = json_string(item, "mediaid")
        .or_else(|| json_string(item, "id"))
        .unwrap_or(fallback_id);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(id));
    info.insert(
        "title",
        serde_json::json!(
            json_string(item, "title")
                .map(unescape_html_attribute)
                .unwrap_or_else(|| fallback_id.to_owned())
        ),
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
        "description",
        json_string(item, "description").map(html_text_fragment),
    );
    info.insert_if_some("thumbnail", json_string(item, "image"));
    info.insert_if_some("timestamp", json_i64(item, "pubdate"));
    info.insert_if_some("duration", json_f64(item, "duration"));
    if let Some(tracks) = item.get("tracks").and_then(serde_json::Value::as_array) {
        let mut subtitles = serde_json::Map::new();
        for track in tracks {
            let Some(raw_url) = json_string(track, "file").filter(|value| !value.is_empty()) else {
                continue;
            };
            let language = json_string(track, "label").unwrap_or("en");
            subtitles
                .entry(language.to_owned())
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .expect("NZZ subtitle list")
                .push(serde_json::json!({
                    "url": proto_relative_url(raw_url, "https:")
                }));
        }
        if !subtitles.is_empty() {
            info.insert("subtitles", serde_json::Value::Object(subtitles));
        }
    }
    Some(info)
}
