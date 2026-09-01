fn massengeschmack_media_url(value: &str) -> Option<String> {
    let value = unescape_html_attribute(value).trim().to_owned();
    (!value.is_empty()).then(|| proto_relative_url(&value, "http:"))
}

fn massengeschmack_media_formats(media: &serde_json::Value) -> Vec<serde_json::Value> {
    let Some(sources) = media.as_array() else {
        return Vec::new();
    };
    let mut formats = Vec::new();
    for source in sources {
        let Some(raw_url) = json_string(source, "src") else {
            continue;
        };
        let Some(media_url) = massengeschmack_media_url(raw_url) else {
            continue;
        };
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "");
        let extension = if extension.is_empty() {
            mimetype_extension(json_string(source, "type")).unwrap_or_else(|| "unknown".to_owned())
        } else {
            extension
        };
        if extension.eq_ignore_ascii_case("m3u8") {
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        } else {
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": extension,
                "ext": extension,
            }));
        }
    }
    formats
}

fn massengeschmack_download_formats(
    webpage: &str,
) -> Vec<serde_json::Value> {
    let Ok(matcher) = Regex::new(
        r#"(?is)<a\b[^>]+?\bhref\s*=\s*"((?:https:)?//[^"]+)".*?<strong>(.*?)</strong>.*?<small>\s*(?:(\d+)x(\d+))?\s*\(([\d,]+\s*[GM]iB)\)</small>"#,
    ) else {
        return Vec::new();
    };
    matcher
        .captures_iter(webpage)
        .flatten()
        .filter_map(|captures| {
            let media_url = captures
                .get(1)
                .map(|value| proto_relative_url(&unescape_html_attribute(value.as_str()), "http:"))?;
            let format_id = captures
                .get(2)
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty())?;
            let filesize = captures
                .get(5)
                .and_then(|value| massengeschmack_filesize(value.as_str()));
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": format_id,
            });
            if let Some(width) = captures
                .get(3)
                .and_then(|value| value.as_str().parse::<i64>().ok())
            {
                format["width"] = serde_json::json!(width);
            }
            if let Some(height) = captures
                .get(4)
                .and_then(|value| value.as_str().parse::<i64>().ok())
            {
                format["height"] = serde_json::json!(height);
            }
            if let Some(filesize) = filesize {
                format["filesize"] = serde_json::json!(filesize);
            }
            if format_id.starts_with("Audio") {
                format["vcodec"] = serde_json::json!("none");
            }
            Some(format)
        })
        .collect()
}

fn massengeschmack_filesize(value: &str) -> Option<i64> {
    let normalized = value
        .replace(',', "")
        .replace(' ', "")
        .strip_suffix("iB")
        .map(str::to_owned)?;
    u64::try_from(yt_dlp_core::parse_bytes(&normalized)?)
        .ok()
        .and_then(|value| i64::try_from(value).ok())
}
