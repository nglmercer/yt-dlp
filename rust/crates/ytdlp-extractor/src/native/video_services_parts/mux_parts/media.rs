fn mux_manifest_url(video_id: &str, token: Option<&str>) -> String {
    let mut manifest = url::Url::parse(&format!("https://stream.mux.com/{video_id}.m3u8"))
        .expect("static Mux manifest URL");
    if let Some(token) = token.filter(|token| !token.is_empty()) {
        manifest.query_pairs_mut().append_pair("token", token);
    }
    manifest.to_string()
}

fn mux_formats(video_id: &str, token: Option<&str>) -> Vec<serde_json::Value> {
    vec![serde_json::json!({
        "url": mux_manifest_url(video_id, token),
        "format_id": "hls",
        "ext": "mp4",
        "protocol": "m3u8_native",
    })]
}
