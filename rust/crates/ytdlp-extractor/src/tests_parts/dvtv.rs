#[test]
fn dvtv_native_extractor_maps_single_player_and_track_variants() {
    let extractor = DvtvExtractor::new(ExtractorDescriptor::new(
        "DVTVIE",
        "dvtv",
        r"https?://video\.aktualne\.cz/(?:[^/]+/)+r~(?P<id>[0-9a-f]{32})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
            <meta property="article:published_time" content="2017-05-11T10:50:00Z">
        </head><body><script>
            BBXPlayer.setup({title: 'Native DVTV', mediaid: 'native-video',
                description: 'Native description', image: '//cdn.example/poster.jpg',
                duration: '42', tracks: {main: [
                    {src: 'https://cdn.example/video.mp4', type: 'video/mp4', label: '720p'},
                    {src: 'https://cdn.example/master.m3u8', type: 'application/vnd.apple.mpegurl'}
                ]}});
        </script></body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://video.aktualne.cz/dvtv/native/r~e5efe9ca855511e4833a0025900fea04/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-video"));
    assert_eq!(result.get_str("title"), Some("Native DVTV"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/poster.jpg"));
    assert_eq!(result.get_i64("duration"), Some(42));
    assert_eq!(result.get_i64("timestamp"), Some(1_494_499_800));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("height")),
        Some(&serde_json::json!(720))
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn dvtv_native_extractor_maps_repeated_playlist_objects() {
    let extractor = DvtvExtractor::new(ExtractorDescriptor::new(
        "DVTVIE",
        "dvtv",
        r"https?://video\.aktualne\.cz/(?:[^/]+/)+r~(?P<id>[0-9a-f]{32})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head><meta name="twitter:title" content="Native playlist"></head><body>
            playlist.push({title: 'First', mediaid: 'first', tracks: {main: [{src: 'https://cdn.example/first.mp4', type: 'video/mp4'}]}});
            playlist.push({title: 'Second', mediaid: 'second', tracks: {main: [{src: 'https://cdn.example/second.mp4', type: 'video/mp4'}]}});
        </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://video.aktualne.cz/dvtv/native/r~973eb3bc854e11e498be002590604f2e/",
            &context,
        )
        .unwrap()
    else {
        panic!("expected DVTV playlist result");
    };

    assert_eq!(info.get_str("id"), Some("973eb3bc854e11e498be002590604f2e"));
    assert_eq!(info.get_str("title"), Some("Native playlist"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("first"));
    assert_eq!(entries[1].get_str("title"), Some("Second"));
}

#[test]
fn dvtv_native_extractor_marks_unparsed_javascript_as_todo() {
    let extractor = DvtvExtractor::new(ExtractorDescriptor::new(
        "DVTVIE",
        "dvtv",
        r"https?://video\.aktualne\.cz/(?:[^/]+/)+r~(?P<id>[0-9a-f]{32})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>BBXPlayer.setup({title: htmldeentitize('Native'), tracks: {main: []}});</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://video.aktualne.cz/dvtv/native/r~e5efe9ca855511e4833a0025900fea04/",
            &context,
        )
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
