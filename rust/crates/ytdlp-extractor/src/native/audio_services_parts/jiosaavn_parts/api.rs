const JIOSAAVN_API_URL: &str = "https://www.jiosaavn.com/api.php";
const JIOSAAVN_DEFAULT_BITRATES: &[&str] = &["128", "320"];

fn jiosaavn_call_api(
    context: &ExtractionContext,
    type_name: &str,
    token: &str,
    extra: &[(&str, String)],
) -> Result<serde_json::Value, ExtractorError> {
    let mut query = vec![
        ("__call".to_owned(), "webapi.get".to_owned()),
        ("_format".to_owned(), "json".to_owned()),
        ("_marker".to_owned(), "0".to_owned()),
        ("ctx".to_owned(), "web6dot0".to_owned()),
        ("token".to_owned(), token.to_owned()),
        ("type".to_owned(), type_name.to_owned()),
    ];
    for (key, value) in extra {
        if let Some(existing) = query.iter_mut().find(|(name, _)| name == key) {
            existing.1.clone_from(value);
        } else {
            query.push(((*key).to_owned(), value.clone()));
        }
    }
    let mut request = Request::new(JIOSAAVN_API_URL);
    request.update_query(&query);
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid JioSaavn {type_name} JSON for {token}: {error}"),
        )
    })
}

fn jiosaavn_call_auth(
    context: &ExtractionContext,
    item_id: &str,
    bitrate: &str,
    encrypted_media_url: &str,
) -> Result<Option<serde_json::Value>, ExtractorError> {
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("__call", "song.generateAuthToken");
    form.append_pair("_format", "json");
    form.append_pair("bitrate", bitrate);
    form.append_pair("url", encrypted_media_url);

    let mut request = Request::new(JIOSAAVN_API_URL);
    request.set_method("POST").map_err(map_request_error)?;
    request.set_data(Some(form.finish().into_bytes()));
    let response = context.request(&request).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Network,
            format!("JioSaavn format request failed for {item_id} at {bitrate} kbps: {error}"),
        )
    })?;
    let data = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid JioSaavn format JSON for {item_id} at {bitrate} kbps: {error}"),
        )
    })?;
    Ok(Some(data))
}

fn jiosaavn_format_list(
    context: &ExtractionContext,
    item: &serde_json::Value,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let item_id = json_value_string(item.get("id")).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "JioSaavn item has no media ID",
        )
    })?;
    let encrypted_media_url = json_string(item, "encrypted_media_url")
        .or_else(|| {
            item.get("more_info")
                .and_then(|more_info| json_string(more_info, "encrypted_media_url"))
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JioSaavn item {item_id} has no encrypted media URL"),
            )
        })?;
    let mut formats = Vec::new();
    for bitrate in JIOSAAVN_DEFAULT_BITRATES {
        let Ok(Some(media_data)) = jiosaavn_call_auth(
            context,
            &item_id,
            bitrate,
            encrypted_media_url,
        ) else {
            continue;
        };
        let Some(media_url) = json_string(&media_data, "auth_url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        else {
            continue;
        };
        let media_type = json_string(&media_data, "type").unwrap_or("mp3");
        let extension = if media_type.eq_ignore_ascii_case("mp4") {
            "m4a"
        } else {
            media_type
        };
        formats.push(serde_json::json!({
            "url": media_url,
            "ext": extension,
            "format_id": bitrate,
            "abr": bitrate.parse::<i64>().unwrap_or_default(),
            "vcodec": "none",
        }));
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("JioSaavn item {item_id} has no authorized audio formats"),
        ));
    }
    Ok(formats)
}
