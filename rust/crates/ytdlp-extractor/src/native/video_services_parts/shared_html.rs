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
