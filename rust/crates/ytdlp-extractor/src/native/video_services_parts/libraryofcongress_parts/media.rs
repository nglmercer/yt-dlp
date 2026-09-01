fn loc_normalize_media_url(raw_url: &str, is_video: bool) -> String {
    let mut media_url = raw_url.replace("rtmp", "https");
    let extension = yt_dlp_core::determine_ext(Some(&media_url), "unknown").to_ascii_lowercase();
    if extension != "mp4" && extension != "mp3" {
        media_url.push_str(if is_video { ".mp4" } else { ".mp3" });
    }
    media_url
}

fn loc_http_url(media_url: &str) -> String {
    Regex::new(r#"(://[^/]+/)(?:[^/]+/)*(?:mp4|mp3):"#)
        .ok()
        .map(|matcher| matcher.replace(media_url, "$1").into_owned())
        .unwrap_or_else(|| media_url.to_owned())
}

fn loc_formats(webpage: &str, media_url: &str, is_video: bool) -> Vec<serde_json::Value> {
    let mut formats = Vec::new();
    if media_url.contains("/vod/mp4:") {
        formats.push(serde_json::json!({
            "url": format!(
                "{}.m3u8",
                media_url.replace("/vod/mp4:", "/hls-vod/media/")
            ),
            "format_id": "hls",
            "ext": "mp4",
            "protocol": "m3u8_native",
            "quality": 1,
        }));
    }

    let mut http_format = serde_json::json!({
        "url": loc_http_url(media_url),
        "format_id": "http",
        "quality": 1,
    });
    if !is_video {
        http_format["vcodec"] = serde_json::json!("none");
    }
    formats.push(http_format);

    let Ok(matcher) = Regex::new(
        r#"(?is)<option[^>]+\bvalue\s*=\s*["'](?P<url>.+?)["'][^>]+\bdata-file-download\s*=\s*[^>]*>\s*(?P<id>.+?)(?:(?:&nbsp;|\s+)\((?P<size>.+?)\))?\s*<"#,
    ) else {
        return formats;
    };
    let mut download_urls = Vec::new();
    for captures in matcher.captures_iter(webpage).flatten() {
        let Some(download_url) = captures
            .name("url")
            .map(|value| unescape_html_attribute(value.as_str()))
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if download_urls.contains(&download_url) {
            continue;
        }
        let format_id = captures
            .name("id")
            .map(|value| html_text_fragment(value.as_str()).trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        let Some(format_id) = format_id else {
            continue;
        };
        if format_id == "gif" || format_id == "jpeg" {
            continue;
        }
        download_urls.push(download_url.clone());
        let mut format = serde_json::json!({
            "url": download_url,
            "format_id": format_id,
        });
        if let Some(filesize) = captures
            .name("size")
            .and_then(|value| loc_parse_filesize(value.as_str()))
        {
            format["filesize_approx"] = serde_json::json!(filesize);
        }
        formats.push(format);
    }
    formats
}
