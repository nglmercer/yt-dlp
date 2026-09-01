#[test]
fn canalalpha_native_extractor_maps_server_state_formats_and_metadata() {
    let extractor = CanalAlphaExtractor::new(ExtractorDescriptor::new(
        "CanalAlphaIE",
        "CanalAlpha",
        r"https?://(?:www\.)?canalalpha\.ch/play/[^/]+/[^/]+/(?P<id>\d+)/?.*",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "canalalpha.ch/play/le-journal/episode/24520".to_owned(),
            br#"<script>window.__SERVER_STATE__ = {
                "1": {"data": {"data": {
                    "title": " Jeudi 28 octobre 2021 ",
                    "longDesc": "<p>Native Canal Alpha description</p>",
                    "poster": "https://cdn.example/canalalpha/poster.jpg",
                    "webPublishAt": "2021-10-28T08:00:00Z",
                    "video": {
                        "duration": 1125,
                        "mp4": [{"$url":"https://cdn.example/canalalpha/360.mp4",
                            "res":{"width":640,"height":360}}],
                        "manifests": {
                            "hls":"https://cdn.example/canalalpha/master.m3u8",
                            "dash":"https://cdn.example/canalalpha/manifest.mpd"
                        }
                    }
                }}}
            }};</script>"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.canalalpha.ch/play/le-journal/episode/24520/jeudi-28-octobre-2021",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("24520"));
    assert_eq!(result.get_str("title"), Some("Jeudi 28 octobre 2021"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Canal Alpha description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/canalalpha/poster.jpg")
    );
    assert_eq!(result.get_str("upload_date"), Some("20211028"));
    assert_eq!(result.get_i64("duration"), Some(1125));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/canalalpha/360.mp4")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("width"), Some(&serde_json::json!(640)));
    assert_eq!(formats[0].get("height"), Some(&serde_json::json!(360)));
    assert_eq!(formats[1].get("format_id"), Some(&serde_json::json!("hls")));
    assert_eq!(
        formats[2].get("protocol"),
        Some(&serde_json::json!("http_dash_segments"))
    );
}

#[test]
fn canalalpha_native_extractor_marks_unknown_manifest_types_as_todo() {
    let extractor = CanalAlphaExtractor::new(ExtractorDescriptor::new(
        "CanalAlphaIE",
        "CanalAlpha",
        r"https?://(?:www\.)?canalalpha\.ch/play/[^/]+/[^/]+/(?P<id>\d+)/?.*",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "canalalpha.ch/play/news/topic/99".to_owned(),
            br#"<script>window.__SERVER_STATE__ = {
                "1": {"data": {"data": {
                    "video": {"manifests": {"smooth": "https://cdn.example/video.ism"}}
                }}}
            }};</script>"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://canalalpha.ch/play/news/topic/99/title", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
