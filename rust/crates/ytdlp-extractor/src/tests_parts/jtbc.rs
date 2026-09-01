#[test]
fn jtbc_native_extractor_maps_playback_tracks_and_details() {
    let extractor = JtbcExtractor::new(ExtractorDescriptor::new(
        "JTBCIE",
        "JTBC",
        r#"https?://(?:vod\.jtbc\.co\.kr/player/(?:program|clip)|tv\.jtbc\.co\.kr/(?:replay|trailer|clip)/pr\d+/pm\d+)/(?P<id>(?:ep|vo)\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "vod.jtbc.co.kr/player/program/ep20216321".to_owned(),
                br#"<div data-vod="VO10721192"></div>"#.to_vec(),
            ),
            (
                "api.jtbc.co.kr/vod/VO10721192".to_owned(),
                br#"{
                    "playTime":"00:01:30",
                    "tracks":[{"file":"https://cdn.example/jtbc-en.vtt","label":"English"}],
                    "sources":{"HLS":[{"file":"https://cdn.example/playlist_pd123.m3u8"}]}
                }"#
                .to_vec(),
            ),
            (
                "now-api.jtbc.co.kr/v1/vod/detail?vodFileId=VO10721192".to_owned(),
                br#"{"vodDetail":{
                    "vodTitleView":"Native JTBC title",
                    "programTitle":"Native JTBC series",
                    "episodeContents":"Native JTBC description",
                    "imgFileUrl":"https://cdn.example/jtbc.jpg",
                    "watchAge":15,
                    "broadcastDate":"2023.10.08"
                }}"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://vod.jtbc.co.kr/player/program/ep20216321",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("VO10721192"));
    assert_eq!(result.get_str("display_id"), Some("ep20216321"));
    assert_eq!(result.get_str("title"), Some("Native JTBC title"));
    assert_eq!(result.get_str("series"), Some("Native JTBC series"));
    assert_eq!(result.get_str("release_date"), Some("20231008"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(90.0)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/playlist.m3u8")
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("English"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!("https://cdn.example/jtbc-en.vtt"))
    );
}

#[test]
fn jtbc_program_native_extractor_builds_replay_playlist() {
    let extractor = JtbcProgramExtractor::new(ExtractorDescriptor::new(
        "JTBCProgramIE",
        "JTBC:program",
        r#"https?://(?:vod\.jtbc\.co\.kr/program|tv\.jtbc\.co\.kr/replay)/(?P<id>pr\d+)/(?:replay|pm\d+)/?(?:$|[?#])"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "now-api.jtbc.co.kr/v1/vodClip/programHome/programReplayVodList".to_owned(),
            br#"{"programReplayVodList":[
                {"episodeId":"VO1002"},
                {"episodeId":"VO1001"},
                {"episodeId":"VO1002"}
            ]}"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://tv.jtbc.co.kr/replay/pr10010392/pm10032710",
            &context,
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected JTBC playlist");
    };
    assert_eq!(info.get_str("id"), Some("pr10010392"));
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://vod.jtbc.co.kr/player/program/VO1001")
    );
    assert_eq!(entries[0].get_str("ie_key"), Some("JTBC"));
}
