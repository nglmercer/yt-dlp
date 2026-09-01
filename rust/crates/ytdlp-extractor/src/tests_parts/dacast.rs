#[test]
fn dacast_vod_native_extractor_maps_api_metadata_and_hls() {
    let extractor = DacastVodExtractor::new(ExtractorDescriptor::new(
        "DacastVODIE",
        "DacastVOD",
        r"https?://iframe\.dacast\.com/vod/(?P<user_id>[\w-]+)/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "playback.dacast.com/content/info".to_owned(),
                br#"{"contentInfo":{"title":"Native Dacast","duration":42.5,"thumbnailUrl":"https://cdn.example/dacast.jpg"}}"#.to_vec(),
            ),
            (
                "playback.dacast.com/content/access".to_owned(),
                br#"{"hls":"https://cdn.example/dacast/master.m3u8"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://iframe.dacast.com/vod/native-user/native-video",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-video"));
    assert_eq!(result.get_str("uploader_id"), Some("native-user"));
    assert_eq!(result.get_str("title"), Some("Native Dacast"));
    assert_eq!(result.get_f64("duration"), Some(42.5));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/dacast.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/dacast/master.m3u8")
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
fn dacast_vod_native_extractor_marks_encrypted_hls_as_todo() {
    let extractor = DacastVodExtractor::new(ExtractorDescriptor::new(
        "DacastVODIE",
        "DacastVOD",
        r"https?://iframe\.dacast\.com/vod/(?P<user_id>[\w-]+)/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "playback.dacast.com/content/info".to_owned(),
                br#"{}"#.to_vec(),
            ),
            (
                "playback.dacast.com/content/access".to_owned(),
                br#"{"hls":"https://cdn.example/uspaes/native-video/master.m3u8"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://iframe.dacast.com/vod/native-user/native-video",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}

#[test]
fn dacast_playlist_native_extractor_maps_vod_entries() {
    let extractor = DacastPlaylistExtractor::new(ExtractorDescriptor::new(
        "DacastPlaylistIE",
        "DacastPlaylist",
        r"https?://iframe\.dacast\.com/playlist/(?P<user_id>[\w-]+)/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "playback.dacast.com/content/info".to_owned(),
                br#"{"contentInfo":{"title":"Native Archive","features":{"playlist":{"contents":[{"id":"native-user-vod-video-one","title":"Episode One"},{"id":"native-user-vod-video-two","title":"Episode Two"}]}}}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://iframe.dacast.com/playlist/native-user/native-playlist",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Dacast playlist result");
    };

    assert_eq!(info.get_str("id"), Some("native-playlist"));
    assert_eq!(info.get_str("title"), Some("Native Archive"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("DacastVOD"));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://iframe.dacast.com/vod/native-user/video-one")
    );
    assert_eq!(entries[0].get_str("title"), Some("Episode One"));
}
