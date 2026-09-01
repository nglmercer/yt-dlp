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

fn json_objects_after_marker(text: &str, marker: &str) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    let mut search_start = 0usize;
    while let Some(marker_offset) = text[search_start..].find(marker) {
        let marker_start = search_start + marker_offset;
        let object_start = marker_start + marker.len();
        let remainder = &text[object_start..];
        let Some(open_offset) = remainder.find('{') else {
            search_start = object_start;
            continue;
        };
        let bytes = remainder.as_bytes();
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut consumed = None;
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
                b'{' => depth = depth.saturating_add(1),
                b'}' => {
                    depth = depth.checked_sub(1).unwrap_or_default();
                    if depth == 0 {
                        consumed = Some(offset + 1);
                        if let Some(value) = parse_common_javascript_value(
                            &String::from_utf8_lossy(&bytes[open_offset..=offset]),
                        ) {
                            values.push(value);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        search_start = object_start + consumed.unwrap_or(remainder.len());
    }
    values
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
    let normalized = normalize_javascript_literal(value)?;
    let matcher = Regex::new(r#"([,{]\s*)([A-Za-z_$][A-Za-z0-9_$-]*)\s*:"#).ok()?;
    let normalized = matcher.replace_all(&normalized, "$1\"$2\":");
    let normalized = remove_javascript_trailing_commas(&normalized);
    serde_json::from_str(&normalized).ok()
}

fn normalize_javascript_literal(value: &str) -> Option<String> {
    let mut normalized = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    while let Some(character) = chars.next() {
        if in_double_quote {
            normalized.push(character);
            if character == '\\' {
                normalized.push(chars.next()?);
            } else if character == '"' {
                in_double_quote = false;
            }
            continue;
        }
        if in_single_quote {
            match character {
                '\\' => {
                    let escaped = chars.next()?;
                    match escaped {
                        '"' => normalized.push_str("\\\""),
                        '\'' => normalized.push('\''),
                        'b' | 'f' | 'n' | 'r' | 't' | 'u' => {
                            normalized.push('\\');
                            normalized.push(escaped);
                        }
                        _ => normalized.push(escaped),
                    }
                }
                '\'' => {
                    normalized.push('"');
                    in_single_quote = false;
                }
                '"' => normalized.push_str("\\\""),
                _ => normalized.push(character),
            }
            continue;
        }
        match character {
            '"' => {
                normalized.push(character);
                in_double_quote = true;
            }
            '\'' => {
                normalized.push('"');
                in_single_quote = true;
            }
            _ => normalized.push(character),
        }
    }
    (!in_double_quote && !in_single_quote).then_some(normalized)
}

fn remove_javascript_trailing_commas(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    let mut in_string = false;
    while let Some(character) = chars.next() {
        if in_string {
            normalized.push(character);
            if character == '\\' {
                if let Some(escaped) = chars.next() {
                    normalized.push(escaped);
                }
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            normalized.push(character);
            continue;
        }
        if character == ',' {
            let mut lookahead = chars.clone();
            while lookahead.peek().is_some_and(|next| next.is_whitespace()) {
                lookahead.next();
            }
            if lookahead.peek().is_some_and(|next| matches!(next, ']' | '}')) {
                continue;
            }
        }
        normalized.push(character);
    }
    normalized
}
