const MEDIASTREAM_EMBED_BASE: &str = "https://mdstrm.com/embed";

fn mediastream_window_value(html: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)window\.{}\s*=\s*["']([^"']+)["']\s*;"#,
        regex::escape(name)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
}

fn mediastream_hls_url(page_url: &str, html: &str, source_url: &str) -> String {
    let mut query = vec![("at".to_owned(), "web-app".to_owned())];
    if let Some(access_token) = url_query_value(page_url, "access_token") {
        query.push(("access_token".to_owned(), access_token));
    }
    for (window_name, query_name) in [
        ("MDSTRMUID", "uid"),
        ("MDSTRMSID", "sid"),
        ("MDSTRMPID", "pid"),
        ("VERSION", "av"),
    ] {
        if let Some(value) = mediastream_window_value(html, window_name) {
            query.push((query_name.to_owned(), value));
        }
    }
    let mut request = Request::new(source_url);
    request.update_query(&query);
    request.url().to_owned()
}

fn mediastream_source_formats(
    page_url: &str,
    html: &str,
    config: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let Some(sources) = config.get("src").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    sources
        .iter()
        .filter_map(|(source_name, value)| {
            let raw_url = value
                .as_str()
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
            let source_url = if source_name == "hls" {
                mediastream_hls_url(page_url, html, raw_url)
            } else {
                raw_url.to_owned()
            };
            let extension = yt_dlp_core::determine_ext(Some(raw_url), "mp4");
            let lower_name = source_name.to_ascii_lowercase();
            let lower_extension = extension.to_ascii_lowercase();
            if source_name == "hls" || lower_name == "hls" || lower_extension == "m3u8" {
                Some(serde_json::json!({
                    "url": source_url,
                    "format_id": source_name,
                    "protocol": "m3u8_native",
                    "ext": "mp4",
                }))
            } else if source_name == "mpd" || lower_name == "mpd" || lower_extension == "mpd" {
                Some(serde_json::json!({
                    "url": source_url,
                    "format_id": source_name,
                    "protocol": "http_dash_segments",
                    "ext": "mp4",
                }))
            } else {
                Some(serde_json::json!({
                    "url": source_url,
                    "format_id": source_name,
                    "protocol": "http",
                    "ext": extension,
                }))
            }
        })
        .collect()
}

fn mediastream_embed_url(value: &str) -> bool {
    Regex::new(r#"^https?://mdstrm\.com/(?:embed|live-stream)/\w+"#)
        .ok()
        .is_some_and(|matcher| matcher.is_match(value).unwrap_or(false))
}

fn mediastream_find_embed_url(html: &str) -> Option<String> {
    if let Some(json_ld) = html_json_ld(html) {
        let is_video_object = json_string(&json_ld, "@type") == Some("VideoObject")
            || json_ld
                .get("@type")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .any(|value| value.as_str() == Some("VideoObject"));
        if is_video_object {
            for key in ["embedUrl", "contentUrl"] {
                if let Some(value) = json_string(&json_ld, key).filter(|value| mediastream_embed_url(value))
                {
                    return Some(value.to_owned());
                }
            }
        }
    }
    let script_matcher = Regex::new(
        r#"(?is)<script[^>]*>[^<]*playerMdStream\.mdstreamVideo\(\s*["'](?P<id>\w+)"#,
    )
    .ok()?;
    if let Some(captures) = script_matcher.captures(html).ok().flatten() {
        if let Some(video_id) = captures.name("id") {
            return Some(format!("{MEDIASTREAM_EMBED_BASE}/{}", video_id.as_str()));
        }
    }
    let iframe_matcher =
        Regex::new(r#"(?is)<iframe[^>]+\bsrc\s*=\s*["'](https?://mdstrm\.com/(?:embed|live-stream)/\w+)"#)
            .ok()?;
    if let Some(captures) = iframe_matcher.captures(html).ok().flatten() {
        if let Some(url) = captures.get(1) {
            return Some(url.as_str().to_owned());
        }
    }
    let player_matcher = Regex::new(
        r#"(?is)<(?:div|ps-mediastream)\b[^>]*\bclass\s*=\s*["'][^"']*MediaStreamVideoPlayer[^"']*["'][^>]*\bdata-video-id\s*=\s*["'](?P<id>\w+)["'][^>]*>"#,
    )
    .ok()?;
    let captures = player_matcher.captures(html).ok().flatten()?;
    let video_id = captures.name("id")?.as_str();
    let live = captures
        .get(0)
        .and_then(|value| {
            Regex::new(r#"(?is)\bdata-video-type\s*=\s*["']live["']"#)
                .ok()
                .map(|matcher| matcher.is_match(value.as_str()).unwrap_or(false))
        })
        .unwrap_or(false);
    Some(format!(
        "https://mdstrm.com/{}/{}",
        if live { "live-stream" } else { "embed" },
        video_id
    ))
}
