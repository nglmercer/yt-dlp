/// Native Vidyard player JSON extractor. The player endpoint exposes direct
/// media, HLS, captions, chapter metadata, and optional additional metadata;
/// multi-chapter players become native playlists.
pub struct VidyardExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

impl VidyardExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let mut matchers = Vec::new();
        for pattern in &descriptor.valid_urls {
            matchers.push(compile_source_pattern(pattern).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Vidyard URL pattern: {error}"),
                )
            })?);
        }
        Ok(Self {
            descriptor,
            matchers,
        })
    }
}

impl InfoExtractor for VidyardExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matchers
            .iter()
            .find_map(|matcher| {
                matcher
                    .captures(url)
                    .ok()
                    .flatten()
                    .and_then(|captures| captures.name("id"))
                    .map(|value| value.as_str().to_owned())
            })
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Vidyard URL has no ID")
            })?;
        let response =
            context.get_json(&format!("https://play.vidyard.com/player/{video_id}.json"))?;
        let payload = response.get("payload").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Vidyard player response has no payload",
            )
        })?;
        let chapters = payload
            .get("chapters")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Vidyard player payload has no chapters",
                )
            })?;
        let mut entries = Vec::new();
        for chapter in chapters {
            let mut entry = vidyard_chapter_info(chapter)?;
            if let Some(facade_id) = json_string(chapter, "facadeUuid") {
                if let Ok(additional) =
                    context.get_json(&format!("https://play.vidyard.com/video/{facade_id}"))
                {
                    merge_vidyard_additional_metadata(&mut entry, &additional);
                }
            }
            entries.push(entry);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Vidyard player {video_id} has no chapters"),
            ));
        }
        if entries.len() == 1 {
            return Ok(ExtractorResult::single(
                entries.pop().expect("one Vidyard chapter"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(
                json_string(payload, "playerUuid")
                    .or_else(|| json_string(payload, "playerUUID"))
                    .unwrap_or(&video_id)
            ),
        );
        info.insert_if_some("title", json_string(payload, "name"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn vidyard_chapter_info(chapter: &serde_json::Value) -> Result<InfoDict, ExtractorError> {
    let facade_id = json_string(chapter, "facadeUuid")
        .or_else(|| json_string(chapter, "id"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Vidyard chapter has no facadeUuid",
            )
        })?;
    let mut formats = Vec::new();
    let sources = chapter.get("sources").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Vidyard chapter has no sources",
        )
    })?;
    if let Some(hls) = sources.get("hls") {
        for source in json_object_values(hls) {
            let Some(media_url) = json_string(source, "url") else {
                continue;
            };
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        }
    }
    if let Some(sources) = sources.as_object() {
        for (source_type, source_list) in sources {
            if source_type == "hls" {
                continue;
            }
            for source in json_object_values(source_list) {
                let Some(media_url) = json_string(source, "url") else {
                    continue;
                };
                let profile = json_string(source, "profile");
                let mut format = serde_json::json!({
                    "url": media_url,
                    "format_id": format!("http-{source_type}{}", profile.map_or_else(String::new, |profile| format!("-{profile}"))),
                    "ext": mimetype_extension(json_string(source, "mimeType"))
                        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(media_url), "mp4")),
                });
                if let Some(profile) = profile {
                    if let Some((width, height)) = parse_resolution_label(profile) {
                        format["width"] = serde_json::json!(width);
                        format["height"] = serde_json::json!(height);
                    } else if let Some(height) = profile
                        .strip_suffix('p')
                        .and_then(|value| value.parse::<i64>().ok())
                    {
                        format["height"] = serde_json::json!(height);
                    }
                }
                formats.push(format);
            }
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Vidyard chapter {facade_id} has no playable sources"),
        ));
    }
    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(facade_id));
    info.insert_if_some(
        "display_id",
        json_i64(chapter, "videoId").map(|value| value.to_string()),
    );
    info.insert_if_some("title", json_string(chapter, "name"));
    info.insert_if_some(
        "description",
        json_string(chapter, "description").map(unescape_html_attribute),
    );
    info.insert_if_some(
        "duration",
        json_f64(chapter, "milliseconds")
            .map(|value| value / 1000.0)
            .or_else(|| json_f64(chapter, "seconds")),
    );
    if let Some(thumbnails) = chapter
        .get("thumbnailUrls")
        .and_then(serde_json::Value::as_object)
    {
        let values = thumbnails
            .values()
            .filter_map(|thumbnail| {
                let url = thumbnail
                    .as_str()
                    .or_else(|| json_string(thumbnail, "url"))?;
                Some(serde_json::json!({"url": url}))
            })
            .collect::<Vec<_>>();
        if !values.is_empty() {
            info.insert("thumbnails", serde_json::Value::Array(values));
        }
    }
    if let Some(captions) = chapter
        .get("captions")
        .and_then(serde_json::Value::as_array)
    {
        let mut subtitles = serde_json::Map::new();
        for caption in captions {
            let Some(url) = json_string(caption, "vttUrl") else {
                continue;
            };
            let language = json_string(caption, "language").unwrap_or("und");
            subtitles
                .entry(language.to_owned())
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .expect("subtitle value is an array")
                .push(serde_json::json!({
                    "url": url,
                    "name": json_string(caption, "name"),
                }));
        }
        if !subtitles.is_empty() {
            info.insert("subtitles", serde_json::Value::Object(subtitles));
        }
    }
    if let Some(tags) = chapter.get("tags").and_then(serde_json::Value::as_array) {
        info.insert(
            "tags",
            serde_json::Value::Array(
                tags.iter()
                    .filter_map(|tag| json_string(tag, "name"))
                    .map(|tag| serde_json::json!(tag))
                    .collect(),
            ),
        );
    }
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
    info.insert(
        "http_headers",
        serde_json::json!({"Referer": "https://play.vidyard.com/"}),
    );
    Ok(info)
}

fn merge_vidyard_additional_metadata(info: &mut InfoDict, metadata: &serde_json::Value) {
    info.insert_if_some(
        "title",
        json_string(metadata, "title").or_else(|| json_string(metadata, "name")),
    );
    info.insert_if_some("duration", json_f64(metadata, "seconds"));
    if let Some(thumbnails) = metadata
        .get("thumbnailUrl")
        .and_then(serde_json::Value::as_object)
        .and_then(|value| value.get("url"))
        .and_then(serde_json::Value::as_str)
    {
        info.insert("thumbnails", serde_json::json!([{"url": thumbnails}]));
    }
    if let Some(sections) = metadata
        .get("videoSections")
        .and_then(serde_json::Value::as_array)
    {
        let chapters = sections
            .iter()
            .filter_map(|section| {
                Some(serde_json::json!({
                    "title": json_string(section, "title")?,
                    "start_time": json_f64(section, "milliseconds").map(|value| value / 1000.0)?,
                }))
            })
            .collect::<Vec<_>>();
        if !chapters.is_empty() {
            info.insert("chapters", serde_json::Value::Array(chapters));
        }
    }
}

fn json_object_values(value: &serde_json::Value) -> Vec<&serde_json::Value> {
    match value {
        serde_json::Value::Array(values) => values.iter().collect(),
        serde_json::Value::Object(values) => values.values().collect(),
        _ => Vec::new(),
    }
}

fn mimetype_extension(mimetype: Option<&str>) -> Option<String> {
    Some(
        match mimetype? {
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "video/ogg" => "ogv",
            "audio/mpeg" => "mp3",
            "audio/mp4" => "m4a",
            "audio/webm" => "webm",
            "audio/ogg" => "ogg",
            "audio/flac" => "flac",
            _ => return None,
        }
        .to_owned(),
    )
}

fn descriptor_matcher(descriptor: &ExtractorDescriptor) -> Result<Regex, ExtractorError> {
    let pattern = descriptor.valid_urls.first().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("native extractor {} has no URL pattern", descriptor.key),
        )
    })?;
    compile_source_pattern(pattern).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid native URL pattern for {}: {error}", descriptor.key),
        )
    })
}

fn proto_relative_url(value: &str, scheme: &str) -> String {
    value
        .strip_prefix("//")
        .map_or_else(|| value.to_owned(), |rest| format!("{scheme}//{rest}"))
}

fn url_query_value(url: &str, key: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()?
        .query_pairs()
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
}

fn date_digits(value: &str) -> Option<String> {
    let digits = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .take(8)
        .collect::<String>();
    (digits.len() == 8).then_some(digits)
}

fn native_url_result(url: &str) -> InfoDict {
    let mut info = InfoDict::new();
    info.insert("_type", serde_json::json!("url"));
    info.insert("url", serde_json::json!(url));
    info
}

fn html5_media_formats(page_url: &str, html: &str) -> Vec<serde_json::Value> {
    let Ok(matcher) = Regex::new(r#"(?is)<(?:source|video|audio)\b[^>]*\bsrc\s*=\s*["']([^"']+)"#)
    else {
        return Vec::new();
    };
    let base_url = url::Url::parse(page_url).ok();
    let mut urls = Vec::new();
    for captures in matcher.captures_iter(html).flatten() {
        let Some(raw_url) = captures.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };
        if raw_url.is_empty() {
            continue;
        }
        let raw_url = proto_relative_url(raw_url, "https:");
        let media_url = base_url
            .as_ref()
            .and_then(|base| base.join(&raw_url).ok())
            .map_or(raw_url, |value| value.to_string());
        if !urls.contains(&media_url) {
            urls.push(media_url);
        }
    }
    urls.into_iter()
        .enumerate()
        .map(|(index, media_url)| {
            let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
            serde_json::json!({
                "format_id": format!("html5-{index}"),
                "url": media_url,
                "ext": ext,
                "protocol": if ext == "m3u8" { "m3u8_native" } else { "http" },
            })
        })
        .collect()
}

fn url_with_scheme(value: &str, scheme: &str) -> String {
    if let Ok(mut parsed) = url::Url::parse(value) {
        if parsed.set_scheme(scheme).is_ok() {
            return parsed.to_string();
        }
    }
    value.split_once("://").map_or_else(
        || value.to_owned(),
        |(_, rest)| format!("{scheme}://{rest}"),
    )
}

fn percent_decode(value: &str) -> String {
    fn hex_digit(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
            {
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn rot13_ascii(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' => {
                let offset = character as u8 - b'a';
                (b'a' + (offset + 13) % 26) as char
            }
            'A'..='Z' => {
                let offset = character as u8 - b'A';
                (b'A' + (offset + 13) % 26) as char
            }
            _ => character,
        })
        .collect()
}

fn native_get_json_with_headers(
    context: &ExtractionContext,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(url);
    for (name, value) in headers {
        request.headers_mut().set(*name, *value);
    }
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid JSON from {}: {error}", response.url()),
        )
    })
}

fn decode_json_string(value: &str) -> Option<String> {
    serde_json::from_str(value).ok()
}

fn json_media_urls(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(values) => values.iter().flat_map(json_media_urls).collect(),
        serde_json::Value::Object(values) => {
            let mut urls = Vec::new();
            for key in ["src", "url"] {
                if let Some(value) = values.get(key).and_then(serde_json::Value::as_str) {
                    urls.push(value.to_owned());
                }
            }
            if urls.is_empty() {
                urls.extend(values.values().flat_map(json_media_urls));
            }
            urls
        }
        _ => Vec::new(),
    }
}

fn html_title_value(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<title\b[^>]*>(.*?)</title>"#).ok()?;
    let captures = matcher.captures(html).ok().flatten()?;
    let title = captures
        .get(1)
        .map(|value| html_text_fragment(value.as_str()))?;
    let title = title
        .trim_end_matches(" - Newgrounds")
        .trim_end_matches(" | Newgrounds")
        .trim();
    (!title.is_empty()).then(|| title.to_owned())
}

fn html_attribute_value(html: &str, attribute: &str, expected: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<[^>]+\b{}\s*=\s*["']{}\s*["'][^>]*\bcontent\s*=\s*["']([^"']+)""#,
        regex::escape(attribute),
        regex::escape(expected),
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn parse_timestamp(value: String) -> Option<i64> {
    yt_dlp_core::parse_iso8601(&value)
        .or_else(|| yt_dlp_core::parse_iso8601(&format!("{value}T00:00:00Z")))
}

fn json_object_after_marker(text: &str, marker: &str) -> Option<serde_json::Value> {
    let marker_start = text.find(marker)?;
    let remainder = &text[marker_start + marker.len()..];
    let open_offset = remainder.find('{')?;
    let bytes = remainder.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes.iter().enumerate().skip(open_offset) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'\"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'\"' => in_string = true,
            b'{' => depth = depth.saturating_add(1),
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return parse_common_javascript_value(&String::from_utf8_lossy(
                        &bytes[open_offset..=offset],
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

fn json_array_after_marker(text: &str, marker: &str) -> Option<serde_json::Value> {
    let marker_start = text.find(marker)?;
    let remainder = &text[marker_start + marker.len()..];
    let open_offset = remainder.find('[')?;
    let bytes = remainder.as_bytes();
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes.iter().enumerate().skip(open_offset) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => stack.push(*byte),
            b'}' => {
                if stack.pop() != Some(b'{') {
                    return None;
                }
            }
            b']' => {
                if stack.pop() != Some(b'[') {
                    return None;
                }
                if stack.is_empty() {
                    return parse_common_javascript_value(&String::from_utf8_lossy(
                        &bytes[open_offset..=offset],
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_common_javascript_value(value: &str) -> Option<serde_json::Value> {
    if let Ok(parsed) = serde_json::from_str(value) {
        return Some(parsed);
    }
    let matcher = Regex::new(r#"([,{]\s*)([A-Za-z_$][A-Za-z0-9_$-]*)\s*:"#).ok()?;
    let normalized = matcher.replace_all(value, "$1\"$2\":");
    serde_json::from_str(&normalized).ok()
}

fn html_json_ld(html: &str) -> Option<serde_json::Value> {
    let matcher = Regex::new(
        r#"(?is)<script\b[^>]*\btype\s*=\s*["']application/ld\+json["'][^>]*>(.*?)</script>"#,
    )
    .ok()?;
    matcher.captures_iter(html).flatten().find_map(|captures| {
        captures
            .get(1)
            .and_then(|value| serde_json::from_str(value.as_str().trim()).ok())
    })
}

fn html_json_number(html: &str, key: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)["']{}\s*["']\s*:\s*["']?([0-9]+(?:\.[0-9]+)?)"#,
        regex::escape(key)
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn html_element_by_id(html: &str, id: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<([a-z0-9]+)\b[^>]*\bid\s*=\s*["']{}\s*["'][^>]*>(.*?)</\1\s*>"#,
        regex::escape(id)
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(2).map(|value| value.as_str().to_owned()))
}

fn path_segment_after(url: &str, marker: &str) -> Result<String, ExtractorError> {
    let parsed = url::Url::parse(url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("invalid extractor URL: {error}"),
        )
    })?;
    let segments = parsed
        .path_segments()
        .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "URL has no path"))?
        .collect::<Vec<_>>();
    let position = segments
        .iter()
        .position(|segment| *segment == marker)
        .and_then(|position| segments.get(position + 1))
        .filter(|segment| !segment.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("URL has no path segment after {marker}"),
            )
        })?;
    Ok((*position).to_owned())
}

fn last_path_segment(url: &str) -> Result<String, ExtractorError> {
    let parsed = url::Url::parse(url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("invalid extractor URL: {error}"),
        )
    })?;
    parsed
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "URL has no ID"))
}

fn html_meta_value(html: &str, key: &str) -> Option<String> {
    let key = regex::escape(key);
    let patterns = [
        format!(
            r#"(?is)<meta\b[^>]*(?:property|name)\s*=\s*["']{key}["'][^>]*content\s*=\s*["']([^"']*)"#,
        ),
        format!(
            r#"(?is)<meta\b[^>]*content\s*=\s*["']([^"']*)["'][^>]*(?:property|name)\s*=\s*["']{key}["']"#,
        ),
    ];
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
    })
}

fn html_script_json(html: &str, script_id: &str) -> Result<serde_json::Value, ExtractorError> {
    let pattern = format!(
        r#"(?is)<script\b[^>]*\bid\s*=\s*["']{}["'][^>]*>(.*?)</script>"#,
        regex::escape(script_id)
    );
    let matcher = Regex::new(&pattern).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid script-data matcher: {error}"),
        )
    })?;
    let captures = matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HTML has no {script_id} JSON script"),
            )
        })?;
    serde_json::from_str(captures.trim()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid {script_id} JSON: {error}"),
        )
    })
}

fn json_string<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn json_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn json_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn json_bool(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(serde_json::Value::as_bool)
}

fn native_post_json(
    context: &ExtractionContext,
    url: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(url);
    request.set_method("POST").map_err(map_request_error)?;
    request.headers_mut().set("Accept", "application/json");
    request
        .headers_mut()
        .set("Content-Type", "application/json");
    request.set_data(Some(serde_json::to_vec(payload).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("could not encode native JSON request: {error}"),
        )
    })?));
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid JSON from {}: {error}", response.url()),
        )
    })
}

fn unescape_html_attribute(value: &str) -> String {
    [
        ("&quot;", "\""),
        ("&#34;", "\""),
        ("&#x22;", "\""),
        ("&#39;", "'"),
        ("&#x27;", "'"),
        ("&apos;", "'"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&nbsp;", " "),
        ("&amp;", "&"),
    ]
    .into_iter()
    .fold(value.to_owned(), |value, (from, to)| {
        value.replace(from, to)
    })
}

fn html_data_json_attribute(html: &str, attribute: &str) -> Option<serde_json::Value> {
    let attribute = regex::escape(attribute);
    for pattern in [
        format!(r#"(?is)\bdata-{attribute}\s*=\s*"([^"]*)"#),
        format!(r#"(?is)\bdata-{attribute}\s*=\s*'([^']*)"#),
    ] {
        let Ok(matcher) = Regex::new(&pattern) else {
            continue;
        };
        let Some(captures) = matcher.captures(html).ok().flatten() else {
            continue;
        };
        let Some(raw) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        if let Ok(value) = serde_json::from_str(&unescape_html_attribute(raw)) {
            return Some(value);
        }
    }
    None
}

fn audio_boom_clip_store(html: &str) -> Option<serde_json::Value> {
    for pattern in [
        r#"(?is)data-react-class\s*=\s*["']V5DetailPagePlayer["'][^>]*data-react-props\s*=\s*["']([^"']*)"#,
        r#"(?is)data-react-props\s*=\s*["']([^"']*)[^>]*data-react-class\s*=\s*["']V5DetailPagePlayer["']"#,
    ] {
        let Ok(matcher) = Regex::new(pattern) else {
            continue;
        };
        let Some(captures) = matcher.captures(html).ok().flatten() else {
            continue;
        };
        let Some(raw) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        if let Ok(store) = serde_json::from_str(&unescape_html_attribute(raw)) {
            return Some(store);
        }
    }
    None
}

fn html_text_fragment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    unescape_html_attribute(output.trim())
}
