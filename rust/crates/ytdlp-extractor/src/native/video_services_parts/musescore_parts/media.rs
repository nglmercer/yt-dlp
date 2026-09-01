fn musescore_formats(audio_url: String) -> Vec<serde_json::Value> {
    vec![serde_json::json!({
        "url": audio_url,
        "format_id": "mp3",
        "ext": "mp3",
        "vcodec": "none",
        "acodec": "mp3",
    })]
}

fn musescore_meta(webpage: &str, key: &str) -> Option<String> {
    html_meta_value(webpage, key)
        .map(|value| html_text_fragment(&value))
        .filter(|value| !value.is_empty())
}
