fn likee_video_url(info: &serde_json::Value) -> Option<String> {
    json_string(info, "video_url")
        .or_else(|| {
            info.get("originVideoInfo")
                .and_then(|value| json_string(value, "video_url"))
        })
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn likee_formats(
    video_url: &str,
    width: Option<i64>,
    height: Option<i64>,
) -> Vec<serde_json::Value> {
    let mut watermarked = serde_json::json!({
        "format_id": "mp4-with-watermark",
        "url": video_url,
    });
    let mut clean = serde_json::json!({
        "format_id": "mp4-without-watermark",
        "url": video_url.replace("_4", ""),
        "quality": 1,
    });
    for format in [&mut watermarked, &mut clean] {
        if let Some(width) = width {
            format["width"] = serde_json::json!(width);
        }
        if let Some(height) = height {
            format["height"] = serde_json::json!(height);
        }
    }
    vec![watermarked, clean]
}
