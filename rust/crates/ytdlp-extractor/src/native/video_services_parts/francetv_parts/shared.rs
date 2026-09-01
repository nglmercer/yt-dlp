const FRANCE_TV_API: &str = "https://k7.ftven.fr/videos";

fn francetv_fetch_json(
    context: &ExtractionContext,
    video_id: &str,
    device_type: &str,
    browser: &str,
) -> Result<Option<serde_json::Value>, ExtractorError> {
    let mut request = Request::new(format!("{FRANCE_TV_API}/{video_id}"));
    request.update_query(&[
        ("device_type".to_owned(), device_type.to_owned()),
        ("browser".to_owned(), browser.to_owned()),
        ("domain".to_owned(), "www.france.tv".to_owned()),
    ]);
    let response = context.request_with_status(&request, &[422])?;
    if response.status() == 422 && response.body().is_empty() {
        return Ok(None);
    }
    serde_json::from_slice(response.body())
        .map(Some)
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!(
                    "invalid FranceTV {device_type}/{browser} JSON for {video_id}: {error}"
                ),
            )
        })
}

fn francetv_tokenized_url(
    context: &ExtractionContext,
    token_url: &str,
    video_url: &str,
) -> Option<String> {
    let mut request = Request::new(token_url);
    request.update_query(&[
        ("format".to_owned(), "json".to_owned()),
        ("url".to_owned(), video_url.to_owned()),
    ]);
    context
        .request(&request)
        .ok()
        .and_then(|response| serde_json::from_slice::<serde_json::Value>(response.body()).ok())
        .and_then(|data| json_string(&data, "url").map(str::to_owned))
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
}

fn francetv_video_token(video: &serde_json::Value) -> Option<String> {
    match video.get("token") {
        Some(serde_json::Value::String(value))
            if value.starts_with("http://") || value.starts_with("https://") =>
        {
            Some(value.clone())
        }
        Some(value) => json_string(value, "akamai")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned),
        None => None,
    }
}

fn francetv_format(
    context: &ExtractionContext,
    video: &serde_json::Value,
    video_id: &str,
) -> Result<Option<serde_json::Value>, ExtractorError> {
    let Some(original_url) = json_string(video, "url")
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
    else {
        return Ok(None);
    };
    let media_url = francetv_video_token(video)
        .and_then(|token_url| francetv_tokenized_url(context, &token_url, original_url))
        .unwrap_or_else(|| original_url.to_owned());
    let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4").to_ascii_lowercase();
    let format_id = json_string(video, "format")
        .filter(|value| !value.is_empty())
        .unwrap_or("http");
    let (protocol, ext) = match extension.as_str() {
        "m3u8" => ("m3u8_native", "mp4"),
        "mpd" => ("http_dash_segments", "mp4"),
        "f4m" => {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: FranceTV video {video_id} requires native Adobe HDS/F4M parsing"
                ),
            ));
        }
        _ if media_url.starts_with("rtmp") => {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: FranceTV video {video_id} requires native RTMP support"),
            ));
        }
        _ => ("http", extension.as_str()),
    };
    Ok(Some(serde_json::json!({
        "url": media_url,
        "format_id": format_id,
        "protocol": protocol,
        "ext": ext,
    })))
}

fn francetv_json_meta(
    meta: &serde_json::Value,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
) {
    let title = json_string(meta, "title").map(str::to_owned);
    let subtitle = json_string(meta, "additional_title").map(str::to_owned);
    let image = json_string(meta, "image_url").map(str::to_owned);
    let timestamp = json_string(meta, "broadcasted_at")
        .map(str::to_owned)
        .and_then(parse_timestamp);
    let (season_number, episode_number) = json_string(meta, "pre_title")
        .and_then(|pre_title| {
            Regex::new(r#"S(\d+)\s*E(\d+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(pre_title).ok().flatten())
        })
        .map(|captures| {
            (
                captures
                    .get(1)
                    .and_then(|value| value.as_str().parse::<i64>().ok()),
                captures
                    .get(2)
                    .and_then(|value| value.as_str().parse::<i64>().ok()),
            )
        })
        .unwrap_or((None, None));
    (
        title,
        subtitle,
        image,
        timestamp,
        season_number,
        episode_number,
    )
}

fn francetv_join_title(title: Option<String>, subtitle: Option<String>) -> Option<String> {
    match (
        title.filter(|value| !value.trim().is_empty()),
        subtitle.filter(|value| !value.trim().is_empty()),
    ) {
        (Some(title), Some(subtitle)) => Some(format!("{title} - {subtitle}")),
        (Some(title), None) => Some(title),
        (None, Some(subtitle)) => Some(subtitle),
        (None, None) => None,
    }
}

fn francetv_video_id_from_page(html: &str) -> Option<String> {
    let patterns = [
        r#"(?is)<(?:button|div)\b[^>]*\bdata-cy\s*=\s*["']francetv-player-wrapper["'][^>]*\bid\s*=\s*["']([^"']+)"#,
        r#"(?is)<(?:button|div)\b[^>]*\bid\s*=\s*["']([^"']+)["'][^>]*\bdata-cy\s*=\s*["']francetv-player-wrapper["']"#,
        r#"(?is)player\.load[^;]+\bsrc\s*:\s*["']([^"']+)"#,
        r#"(?is)\bid-video\s*=\s*["']([^"']+)"#,
        r#"(?is)<figure[^>]+\bid\s*=\s*["']([\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12})"#,
    ];
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
            .and_then(|value| francetv_normalize_page_video_id(&value))
    })
}

fn francetv_normalize_page_video_id(value: &str) -> Option<String> {
    let value = unescape_html_attribute(value);
    let value = value
        .split('?')
        .next()
        .unwrap_or(&value)
        .trim_end_matches('/');
    let value = value.rsplit_once("/video/").map_or(value, |(_, value)| value);
    let value = value.split('/').next().unwrap_or(value);
    (!value.is_empty()).then(|| value.to_owned())
}

fn francetv_next_options_id(html: &str) -> Option<String> {
    let matcher =
        Regex::new(r#"(?is)<script\b[^>]*>self\.__next_f\.push\((\[.+?\])\)</script>"#).ok()?;
    for captures in matcher.captures_iter(html).flatten() {
        let Some(segment) = captures
            .get(1)
            .and_then(|value| parse_common_javascript_value(value.as_str()))
        else {
            continue;
        };
        let Some(payload) = segment.as_array() else {
            continue;
        };
        if payload.first().and_then(serde_json::Value::as_i64) != Some(1) {
            continue;
        }
        let Some(chunk) = payload.get(1).and_then(serde_json::Value::as_str) else {
            continue;
        };
        for line in chunk.lines() {
            let Some((prefix, body)) = line.split_once(':') else {
                continue;
            };
            if !prefix.chars().all(|character| character.is_ascii_hexdigit()) {
                continue;
            }
            let Some(value) = parse_common_javascript_value(body) else {
                continue;
            };
            if let Some(id) = francetv_find_options_id(&value) {
                return Some(id);
            }
        }
    }
    None
}

fn francetv_find_options_id(value: &serde_json::Value) -> Option<String> {
    if let Some(id) = value
        .get("options")
        .and_then(|options| json_string(options, "id"))
    {
        return Some(id.to_owned());
    }
    match value {
        serde_json::Value::Array(values) => values.iter().find_map(francetv_find_options_id),
        serde_json::Value::Object(values) => values.values().find_map(francetv_find_options_id),
        _ => None,
    }
}
