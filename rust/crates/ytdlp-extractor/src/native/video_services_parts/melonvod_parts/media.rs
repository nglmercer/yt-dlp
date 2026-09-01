fn melonvod_hls_format(media_url: String) -> serde_json::Value {
    serde_json::json!({
        "url": media_url,
        "format_id": "hls",
        "ext": "mp4",
        "protocol": "m3u8_native",
    })
}

fn melonvod_thumbnail(static_domain: Option<&str>, image_path: Option<&str>) -> Option<String> {
    let image_path = image_path?.trim();
    (!image_path.is_empty()).then(|| {
        static_domain
            .filter(|domain| !domain.is_empty())
            .map_or_else(|| image_path.to_owned(), |domain| resolve_url(domain, image_path))
    })
}

fn melonvod_artist(play_info: &serde_json::Value) -> Option<String> {
    let artists = play_info
        .get("artistList")
        .and_then(serde_json::Value::as_array)?;
    let artist = artists
        .iter()
        .filter_map(|artist| json_string(artist, "ARTISTNAMEWEBLIST"))
        .filter(|artist| !artist.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    (!artist.is_empty()).then_some(artist)
}

fn melonvod_upload_date(value: Option<&str>) -> Option<String> {
    let value = value?;
    (value.len() >= 8).then(|| value.chars().take(8).collect())
}
