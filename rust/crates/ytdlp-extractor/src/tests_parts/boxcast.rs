#[test]
fn boxcast_native_extractor_maps_preloaded_recorded_broadcast() {
    let extractor = BoxCastExtractor::new(ExtractorDescriptor::new(
        "BoxCastVideoIE",
        "BoxCastVideo",
        r"(?x)
        https?://boxcast\.tv/(?:
            view-embed/|
            channel/\w+\?(?:[^#]+&)?b=|
            video-portal/(?:\w+/){2}
        )(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "boxcast.tv/view-embed/native-broadcast".to_owned(),
            br#"<meta property="og:title" content="Fallback title">
                <script>var BOXCAST_PRELOAD = {
                    "broadcast": {"data": {
                        "id": "native-broadcast-id",
                        "name": "Native BoxCast broadcast",
                        "description": "Native broadcast description",
                        "preview": "https://cdn.example/boxcast/poster.png",
                        "streamed_at": "2022-12-10T00:00:00Z",
                        "account_name": "Native account",
                        "account_id": "native-account"
                    }},
                    "view": {"data": {
                        "status": "recorded",
                        "playlist": "https://cdn.example/boxcast/native.m3u8"
                    }}
                }};</script>"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://boxcast.tv/view-embed/native-broadcast",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-broadcast-id"));
    assert_eq!(
        result.get_str("title"),
        Some("Native BoxCast broadcast")
    );
    assert_eq!(result.get_str("uploader"), Some("Native account"));
    assert_eq!(result.get_str("uploader_id"), Some("native-account"));
    assert_eq!(result.get_i64("release_timestamp"), Some(1_670_630_400));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/boxcast/native.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn boxcast_native_extractor_marks_live_broadcast_as_todo() {
    let extractor = BoxCastExtractor::new(ExtractorDescriptor::new(
        "BoxCastVideoIE",
        "BoxCastVideo",
        r"https?://boxcast\.tv/view-embed/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "boxcast.tv/view-embed/live-broadcast".to_owned(),
            br#"<script>var BOXCAST_PRELOAD = {
                "broadcast": {"data": {"id": "live-broadcast"}},
                "view": {"data": {"status": "live"}}
            };</script>"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://boxcast.tv/view-embed/live-broadcast", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
