fn mux_extractor() -> MuxExtractor {
    MuxExtractor::new(ExtractorDescriptor::new(
        "MuxIE",
        "Mux",
        r#"https?://(?:stream\.new/v|player\.mux\.com)/(?P<id>[A-Za-z0-9-]+)"#,
        true,
    ))
    .unwrap()
}

#[test]
fn mux_native_extractor_maps_stream_new_and_player_urls() {
    for url in [
        "https://stream.new/v/OCtRWZiZqKvLbnZ32WSEYiGNvHdAmB01j/embed",
        "https://player.mux.com/OCtRWZiZqKvLbnZ32WSEYiGNvHdAmB01j",
    ] {
        let result = mux_extractor()
            .extract_with_context(url, &ExtractionContext::native())
            .unwrap()
            .into_info_dict();
        assert_eq!(
            result.get_str("id"),
            Some("OCtRWZiZqKvLbnZ32WSEYiGNvHdAmB01j")
        );
        assert_eq!(
            result.get_str("title"),
            Some("OCtRWZiZqKvLbnZ32WSEYiGNvHdAmB01j")
        );
        assert_eq!(result.get_str("ext"), Some("mp4"));
        assert_eq!(
            result.get_str("url"),
            Some("https://stream.mux.com/OCtRWZiZqKvLbnZ32WSEYiGNvHdAmB01j.m3u8")
        );
        assert_eq!(
            result
                .get("formats")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }
}

#[test]
fn mux_native_extractor_preserves_playback_token_on_manifest() {
    let result = mux_extractor()
        .extract_with_context(
            "https://player.mux.com/native-id?playback-token=signed-token",
            &ExtractionContext::native(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result.get_str("url"),
        Some("https://stream.mux.com/native-id.m3u8?token=signed-token")
    );
}
