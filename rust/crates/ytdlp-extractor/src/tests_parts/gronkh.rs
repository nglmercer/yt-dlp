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

#[test]
fn gronkh_feed_native_extractor_builds_discovery_playlist() {
    let extractor = GronkhFeedExtractor::new(ExtractorDescriptor::new(
        "GronkhFeedIE",
        "gronkh:feed",
        r#"https?://(?:www\.)?gronkh\.tv(?:/feed)?/?(?:#|$)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api.gronkh.tv/v1/video/discovery/recent".to_owned(),
                br#"{"discovery":[{"episode":657,"title":"Recent title"}]}"#.to_vec(),
            ),
            (
                "api.gronkh.tv/v1/video/discovery/views".to_owned(),
                br#"{"discovery":[{"episode":536,"title":"Most viewed"}]}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("https://gronkh.tv/feed", &context)
        .unwrap()
    else {
        panic!("expected Gronkh feed playlist");
    };

    assert_eq!(info.get_str("id"), Some("feed"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("_type"), Some("url"));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://gronkh.tv/watch/stream/657")
    );
    assert_eq!(entries[0].get_str("ie_key"), Some("Gronkh"));
    assert_eq!(entries[0].get_str("id"), Some("Recent title"));
}

#[test]
fn gronkh_vods_native_extractor_pages_search_results() {
    let extractor = GronkhVodsExtractor::new(ExtractorDescriptor::new(
        "GronkhVodsIE",
        "gronkh:vods",
        r#"https?://(?:www\.)?gronkh\.tv/vods/streams/?(?:#|$)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api.gronkh.tv/v1/search?offset=0&first=25".to_owned(),
                br#"{"results":{"videos":[{"episode":657,"title":"First VOD"}]}}"#
                    .to_vec(),
            ),
            (
                "api.gronkh.tv/v1/search?offset=25&first=25".to_owned(),
                br#"{"results":{"videos":[{"episode":536,"title":"Second VOD"}]}}"#
                    .to_vec(),
            ),
            (
                "api.gronkh.tv/v1/search?offset=50&first=25".to_owned(),
                br#"{"results":{"videos":[]}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("https://gronkh.tv/vods/streams", &context)
        .unwrap()
    else {
        panic!("expected Gronkh VOD playlist");
    };

    assert_eq!(info.get_str("id"), Some("vods"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("657"));
    assert_eq!(entries[0].get_str("title"), Some("First VOD"));
    assert_eq!(entries[1].get_str("id"), Some("536"));
    assert_eq!(entries[1].get_str("title"), Some("Second VOD"));
}
