#[test]
fn firsttv_native_extractor_builds_vod_playlist_entries() {
    let extractor = FirstTvExtractor::new(ExtractorDescriptor::new(
        "FirstTVIE",
        "1tv",
        r"https?://(?:www\.)?(?:sport)?1tv\.ru/(?:[^/?#]+/)+(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "1tv.ru/shows/native".to_owned(),
                br#"<html><head>
                    <meta property="og:title" content="Native 1TV show">
                    <meta property="og:image" content="https://cdn.example/1tv.jpg">
                </head><body>
                    <div data-playlist-url="/api/playlist/native?video_id=1001"></div>
                </body></html>"#
                    .to_vec(),
            ),
            (
                "1tv.ru/api/playlist/native".to_owned(),
                br#"[{
                    "id":1001,"uid":1001,"title":"Native episode","poster":"https://cdn.example/episode.jpg",
                    "dvr_begin_at":1700000000,"date_air":"2024-01-02","duration":"120",
                    "sources":[
                        {"src":"https://cdn.example/episode_720.mp4","type":"video/mp4"},
                        {"src":"https://cdn.example/episode.m3u8","type":"application/x-mpegURL"}
                    ],
                    "episodes":[{"from":0,"to":30,"name":"<b>Intro</b>"}]
                },{
                    "id":1002,"uid":1002,"title":"Filtered episode",
                    "sources":[{"src":"https://cdn.example/filtered.mp4","type":"video/mp4"}]
                }]"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.1tv.ru/shows/native/episode",
            &context,
        )
        .unwrap()
    else {
        panic!("1TV VOD should return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("episode"));
    assert_eq!(info.get_str("title"), Some("Native 1TV show"));
    assert_eq!(
        info.get_str("thumbnail"),
        Some("https://cdn.example/1tv.jpg")
    );
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].get_str("id"), Some("1001"));
    assert_eq!(entries[0].get_i64("duration"), Some(120));
    assert_eq!(
        entries[0].get("chapters"),
        Some(&serde_json::json!([{
            "start_time": 0.0,
            "end_time": 30.0,
            "title": "Intro"
        }]))
    );
    assert_eq!(
        entries[0]
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn firsttv_live_native_extractor_maps_dash_manifest() {
    let extractor = FirstTvLiveExtractor::new(ExtractorDescriptor::new(
        "FirstTVLiveIE",
        "1tv:live",
        r"https?://(?:www\.)?1tv\.ru/live",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "1tv.ru/live".to_owned(),
                r#"<html><head><title>ПЕРВЫЙ КАНАЛ ПРЯМОЙ ЭФИР</title></head></html>"#
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "stream.1tv.ru/api/playlist/1tvch-v1_as_array.json".to_owned(),
                br#"{"mpd":["https://cdn.example/1tv/live.mpd"]}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.1tv.ru/live", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("live"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/1tv/live.mpd")
    );
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("http_dash_segments"))
    );
}
