fn mixlr_http_url(value: Option<String>) -> Option<String> {
    let value = value?;
    let value = value.trim();
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn mixlr_extension_from_filename(value: Option<&str>) -> Option<String> {
    let extension = yt_dlp_core::determine_ext(value, "");
    (!extension.is_empty()).then_some(extension)
}

fn mixlr_mimetype_extension(value: &str) -> Option<String> {
    let mimetype = value.split(';').next()?.trim().to_ascii_lowercase();
    mimetype_extension(Some(&mimetype)).or_else(|| {
        let subtype = mimetype.rsplit_once('/')?.1;
        (!subtype.is_empty()).then(|| subtype.replace('+', "."))
    })
}

fn mixlr_detect_ext(response: &yt_dlp_networking::Response, media_url: &str) -> String {
    let content_disposition = response.headers().get("Content-Disposition");
    let filename = Regex::new(r#"(?i)^attachment;\s*filename="([^"]+)""#)
        .ok()
        .and_then(|matcher| {
            matcher
                .captures(content_disposition.unwrap_or_default())
                .ok()
                .flatten()
        })
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()));
    let extension = mixlr_extension_from_filename(filename.as_deref())
        .or_else(|| {
            mixlr_extension_from_filename(response.headers().get("x-amz-meta-name"))
        })
        .or_else(|| {
            response
                .headers()
                .get("x-amz-meta-file-type")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
        .or_else(|| {
            response
                .headers()
                .get("Content-Type")
                .and_then(mixlr_mimetype_extension)
        })
        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(media_url), "mp3"));
    if extension.eq_ignore_ascii_case("octet-stream") {
        "mp3".to_owned()
    } else {
        extension
    }
}

fn mixlr_progressive_format(
    context: &ExtractionContext,
    media_url: &str,
) -> Option<serde_json::Value> {
    let mut request = Request::new(media_url);
    request.set_method("HEAD").ok()?;
    let response = context.request(&request).ok()?;
    if response.status() != 200 {
        return None;
    }
    let extension = mixlr_detect_ext(&response, media_url);
    Some(serde_json::json!({
        "url": media_url,
        "ext": extension,
        "vcodec": "none",
    }))
}
