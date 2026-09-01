#[test]
fn hungama_native_video_extractor_maps_hls_metadata_and_subtitles() {
    let extractor = HungamaExtractor::new(ExtractorDescriptor::new(
        "HungamaIE",
        "Hungama",
        r#"https?://(?:www\.|un\.)?hungama\.com/(?:(?:video|movie|short-film)/[^/]+/|tv-show/(?:[^/]+/){2}\d+/episode/[^/]+/)(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "hungama.com/index.php".to_owned(),
                br#"{"stream_url":"https://cdn.example/hungama/native.m3u8","sub_title":"https://cdn.example/hungama/native.vtt"}"#.to_vec(),
            ),
            (
                "cpage.api.hungama.com/v2/page/content/39349649/movie/detail".to_owned(),
                br#"{"data":{"head":{"data":{"title":"Native Hungama video","description":"Native description","duration":264,"releasedate":"2018-08-29","image":"https://cdn.example/hungama.jpg","misc":{"description":"Native misc description","playcount":42,"keywords":["native","rust"]}}}}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.hungama.com/video/native-video/39349649/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("39349649"));
    assert_eq!(result.get_str("title"), Some("Native Hungama video"));
    assert_eq!(result.get_str("description"), Some("Native misc description"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/hungama/native.m3u8"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1535500800)));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(42)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!("https://cdn.example/hungama/native.vtt"))
    );
}

#[test]
fn hungama_song_native_extractor_maps_media_endpoint() {
    let extractor = HungamaSongExtractor::new(ExtractorDescriptor::new(
        "HungamaSongIE",
        "HungamaSong",
        r#"https?://(?:www\.|un\.)?hungama\.com/song/[^/]+/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "hungama.com/audio-player-data/track/2931166".to_owned(),
                br#"[{"song_name":"Native Song","singer_name":"Native Artist","album_name":"Native Album","date":"2024","img_src":"https://cdn.example/song.jpg","file":"https://media.example/native.json"}]"#.to_vec(),
            ),
            (
                "media.example/native.json".to_owned(),
                br#"{"response":{"media_url":"https://cdn.example/native.mp3","type":"mp3"}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.hungama.com/song/native-song/2931166/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2931166"));
    assert_eq!(result.get_str("title"), Some("Native Artist - Native Song"));
    assert_eq!(result.get_str("track"), Some("Native Song"));
    assert_eq!(result.get_str("artist"), Some("Native Artist"));
    assert_eq!(result.get_str("album"), Some("Native Album"));
    assert_eq!(result.get("release_year"), Some(&serde_json::json!(2024)));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/native.mp3"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
}

#[test]
fn hungama_album_native_extractor_maps_song_entries() {
    let extractor = HungamaAlbumPlaylistExtractor::new(ExtractorDescriptor::new(
        "HungamaAlbumPlaylistIE",
        "HungamaAlbumPlaylist",
        r#"https?://(?:www\.|un\.)?hungama\.com/(?P<path>playlists|album)/[^/]+/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "cpage.api.hungama.com/v2/page/content/123063/playlist/detail".to_owned(),
            br#"{"data":{"body":{"rows":[
                {"data":{"misc":{"share":"https://www.hungama.com/song/native-one/1"}}},
                {"data":{"misc":{"share":"https://www.hungama.com/song/native-two/2"}}}
            ]}}}"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.hungama.com/playlists/native-playlist/123063/",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Hungama playlist");
    };

    assert_eq!(info.get_str("id"), Some("123063"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("HungamaSong"));
    assert_eq!(
        entries[1].get_str("url"),
        Some("https://www.hungama.com/song/native-two/2")
    );
}

#[test]
fn hungama_native_video_extractor_marks_non_hls_stream_as_todo() {
    let extractor = HungamaExtractor::new(ExtractorDescriptor::new(
        "HungamaIE",
        "Hungama",
        r#"https?://(?:www\.|un\.)?hungama\.com/(?:(?:video|movie|short-film)/[^/]+/|tv-show/(?:[^/]+/){2}\d+/episode/[^/]+/)(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"stream_url":"https://cdn.example/hungama/native.mp4"}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://www.hungama.com/video/native-video/39349649/",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
