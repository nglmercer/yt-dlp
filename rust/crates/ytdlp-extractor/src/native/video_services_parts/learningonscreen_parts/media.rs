fn los_formats(page_url: &str, html: &str) -> Vec<serde_json::Value> {
    let mut formats = html5_media_formats(page_url, html);
    for format in &mut formats {
        format["http_headers"] = serde_json::json!({
            "Origin": "https://learningonscreen.ac.uk",
            "Referer": "https://learningonscreen.ac.uk/",
        });
    }
    formats
}
