#[test]
fn gronkh_native_extractor_maps_vod_metadata_hls_and_chapters() {
    let extractor = GronkhExtractor::new(ExtractorDescriptor::new(
        "GronkhIE",
        "Gronkh",
        r#"https?://(?:www\.)?gronkh\.tv/(?:watch/)?streams?/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api.gronkh.tv/v1/video/info?episode=657".to_owned(),
                br#"{"title":"Native Gronkh VOD","views":1234,
                    "preview_url":"https://cdn.example/gronkh.jpg",
                    "created_at":"2022-11-11T08:00:00Z",
                    "source_length":31463,"vtt_url":"https://cdn.example/subs.vtt",
                    "chapters":[{"title":"Intro","offset":0},{"title":"Main","offset":120.5}]}"#
                    .to_vec(),
            ),
            (
                "api.gronkh.tv/v1/video/playlist?episode=657".to_owned(),
                br#"{"playlist_url":"https://cdn.example/gronkh/master.m3u8"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://gronkh.tv/streams/657", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("657"));
    assert_eq!(result.get_str("title"), Some("Native Gronkh VOD"));
    assert_eq!(result.get_i64("view_count"), Some(1234));
    assert_eq!(result.get_str("upload_date"), Some("20221111"));
    assert_eq!(result.get_f64("duration"), Some(31_463.0));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/gronkh/master.m3u8")
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        result
            .get("chapters")
            .and_then(serde_json::Value::as_array)
            .and_then(|chapters| chapters.get(1))
            .and_then(|chapter| chapter.get("start_time"))
            .and_then(serde_json::Value::as_f64),
        Some(120.5)
    );
}
