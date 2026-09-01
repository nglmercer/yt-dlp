fn le_decrypt_m3u8(data: &[u8], _video_id: &str) -> Vec<u8> {
    if data.len() < 5 || !data[..5].eq_ignore_ascii_case(b"vc_01") {
        return data.to_vec();
    }
    let encrypted = &data[5..];
    if encrypted.len() < 6 {
        return data.to_vec();
    }
    let mut expanded = Vec::with_capacity(encrypted.len() * 2);
    for value in encrypted {
        expanded.push(value / 16);
        expanded.push(value % 16);
    }
    let offset = expanded.len() - 11;
    let mut rotated = Vec::with_capacity(expanded.len());
    rotated.extend_from_slice(&expanded[offset..]);
    rotated.extend_from_slice(&expanded[..offset]);
    let mut output = Vec::with_capacity(encrypted.len());
    for pair in rotated.chunks_exact(2) {
        output.push(pair[0] * 16 + pair[1]);
    }
    output
}

fn le_base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = u32::from(*chunk.get(1).unwrap_or(&0));
        let third = u32::from(*chunk.get(2).unwrap_or(&0));
        let value = (first << 16) | (second << 8) | third;
        output.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[((value >> 6) & 63) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(value & 63) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn le_manifest_data_uri(data: &[u8]) -> String {
    format!(
        "data:application/vnd.apple.mpegurl;base64,{}",
        le_base64_encode(data)
    )
}

fn le_format_extension(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "mp4" | "video/mp4" => "mp4".to_owned(),
        "webm" | "video/webm" => "webm".to_owned(),
        "flv" | "video/x-flv" => "flv".to_owned(),
        _ => yt_dlp_core::determine_ext(Some(&value), "mp4"),
    }
}

fn le_dispatch_format(
    context: &ExtractionContext,
    video_id: &str,
    domain: &str,
    format_id: &str,
    format_data: &serde_json::Value,
) -> Result<serde_json::Value, ExtractorError> {
    let path = format_data
        .get(0)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Le format {format_id} for {video_id} has no node path"),
            )
        })?;
    let format_type = format_data
        .get(1)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("mp4");
    let media_url = resolve_url(domain, path);
    let nodes = le_node_json(context, video_id, &media_url)?;
    let location = nodes
        .get("nodelist")
        .and_then(serde_json::Value::as_array)
        .and_then(|nodes| nodes.first())
        .and_then(|node| json_string(node, "location"))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Le format {format_id} for {video_id} has no manifest location"),
            )
        })?;
    let manifest = le_manifest_request(context, video_id, location)?;
    let quality = format_id.parse::<i64>().ok();
    let mut format = serde_json::json!({
        "url": le_manifest_data_uri(&manifest),
        "ext": le_format_extension(format_type),
        "format_id": format!("hls-{format_id}"),
        "protocol": "m3u8_native",
    });
    if let Some(quality) = quality {
        format["quality"] = serde_json::json!(quality);
    }
    if let Some(height) = format_id
        .strip_suffix('p')
        .and_then(|value| value.parse::<i64>().ok())
    {
        format["height"] = serde_json::json!(height);
    }
    Ok(format)
}
