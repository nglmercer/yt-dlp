fn mediaklikk_page(
    context: &ExtractionContext,
    url: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn mediaklikk_player_data(
    webpage: &str,
    display_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    json_object_after_marker(webpage, "loadPlayer(").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("MediaKlikk page {display_id} has no player configuration"),
        )
    })
}

fn mediaklikk_unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = mediaklikk_hex_digit(bytes[index + 1]);
            let low = mediaklikk_hex_digit(bytes[index + 2]);
            if let (Some(high), Some(low)) = (high, low) {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn mediaklikk_hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn mediaklikk_player_json(
    context: &ExtractionContext,
    page_url: &str,
    video_id: &str,
    player_data: &serde_json::Value,
) -> Result<serde_json::Value, ExtractorError> {
    let token = json_string(player_data, "token").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("MediaKlikk video {video_id} has no player token"),
        )
    })?;
    let mut query = Vec::new();
    if let Some(fields) = player_data.as_object() {
        for (key, value) in fields {
            if key == "token" {
                continue;
            }
            let Some(value) = mediaklikk_query_value(value) else {
                continue;
            };
            query.push((key.clone(), value));
        }
    }
    query.push(("video".to_owned(), mediaklikk_unquote(token)));
    let mut request = Request::new("https://player.mediaklikk.hu/playernew/player.php");
    request.update_query(&query);
    request.headers_mut().set("Referer", page_url);
    let response = context.request(&request)?;
    let player_page = String::from_utf8_lossy(response.body());
    json_object_after_marker(&player_page, "pl.setup").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("MediaKlikk player response for {video_id} has no setup JSON"),
        )
    })
}

fn mediaklikk_query_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}
