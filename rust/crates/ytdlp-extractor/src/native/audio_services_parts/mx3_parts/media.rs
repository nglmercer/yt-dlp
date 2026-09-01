const MX3_FORMATS: [(&str, &str, i64); 4] = [
    ("default", "player_asset", 0),
    ("hd", "player_asset?quality=hd", 1),
    ("download", "download", 2),
    ("source", "player_asset?quality=source", 2),
];

fn mx3_content_type_extension(value: &str) -> Option<&'static str> {
    let value = value.split(';').next()?.trim().to_ascii_lowercase();
    match value.as_str() {
        "audio/mpeg" => Some("mp3"),
        "audio/wav" | "audio/wave" | "audio/x-wav" => Some("wav"),
        "audio/mp4" => Some("m4a"),
        "audio/ogg" => Some("ogg"),
        "audio/flac" => Some("flac"),
        "video/mp4" => Some("mp4"),
        "video/quicktime" => Some("mov"),
        "video/webm" => Some("webm"),
        _ => None,
    }
}

fn mx3_header_filename(response: &yt_dlp_networking::Response) -> Option<String> {
    let header = response.headers().get("Content-Disposition")?;
    let matcher = Regex::new(r#"(?i)filename\s*=\s*[\"']?([^\"';]+)"#).ok()?;
    matcher
        .captures(header)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_owned())
}

fn mx3_format_extension(response: &yt_dlp_networking::Response, format_url: &str) -> String {
    mx3_header_filename(response)
        .map(|filename| yt_dlp_core::determine_ext(Some(&filename), ""))
        .filter(|extension| !extension.is_empty())
        .or_else(|| {
            response
                .headers()
                .get("Content-Type")
                .and_then(mx3_content_type_extension)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| {
            let url_extension = yt_dlp_core::determine_ext(Some(format_url), "mp3");
            yt_dlp_core::determine_ext(Some(response.url()), &url_extension)
        })
}

fn mx3_extract_formats(
    context: &ExtractionContext,
    domain: &str,
    track_id: &str,
) -> Vec<serde_json::Value> {
    let mut formats = Vec::new();
    for (format_id, path, quality) in MX3_FORMATS {
        let format_url = format!("https://{domain}/tracks/{track_id}/{path}");
        let mut request = Request::new(&format_url);
        if request.set_method("HEAD").is_err() {
            continue;
        }
        let Ok(response) = context.request_with_status(&request, &[404]) else {
            continue;
        };
        if response.status() != 200 {
            continue;
        }
        let extension = mx3_format_extension(&response, &format_url);
        let mut format = serde_json::json!({
            "url": format_url,
            "format_id": format_id,
            "quality": quality,
            "ext": extension,
        });
        if let Some(filesize) = response
            .headers()
            .get("Content-Length")
            .and_then(|value| value.parse::<i64>().ok())
        {
            format["filesize"] = serde_json::json!(filesize);
        }
        formats.push(format);
    }
    formats
}
