#[test]
fn glomex_embed_native_extractor_maps_api_video() {
    let extractor = GlomexEmbedExtractor::new(ExtractorDescriptor::new(
        "GlomexEmbedIE",
        "glomex:embed",
        r#"https?://player\.glomex\.com/integration/[^/]/iframe\-player\.html\?([^#]+&)?playlistId=(?P<id>[^#&]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "integration-cloudfront-eu-west-1.mes.glomex.cloud/".to_owned(),
                br#"{"videos":[{"clip_id":"v-native","title":"Native Glomex video","description":"A native description","clip_duration":296,"created_at":1619895017,"source":{"hls":"https://cdn.example/native.m3u8","mp4":"https://cdn.example/native.mp4"},"images":[{"id":"hero","url":"https://thumbs.example/native"}]}]}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://player.glomex.com/integration/1/iframe-player.html?integrationId=native&playlistId=v-native",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("v-native"));
    assert_eq!(result.get_str("title"), Some("Native Glomex video"));
    assert_eq!(result.get_i64("duration"), Some(296));
    assert_eq!(result.get_i64("timestamp"), Some(1_619_895_017));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://thumbs.example/native/profile:player-960x540")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn glomex_embed_native_extractor_builds_playlist_and_page_redirects() {
    let embed_extractor = GlomexEmbedExtractor::new(ExtractorDescriptor::new(
        "GlomexEmbedIE",
        "glomex:embed",
        r#"https?://player\.glomex\.com/integration/[^/]/iframe\-player\.html\?([^#]+&)?playlistId=(?P<id>[^#&]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "integration-cloudfront-eu-west-1.mes.glomex.cloud/".to_owned(),
                br#"{"videos":[{"clip_id":"v-first","source":{"mp4":"https://cdn.example/first.mp4"}},{"clip_id":"v-second","source":{"mp4":"https://cdn.example/second.mp4"}}]}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = embed_extractor
        .extract_with_context(
            "https://player.glomex.com/integration/1/iframe-player.html?playlistId=pl-native&integrationId=native",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Glomex playlist");
    };
    assert_eq!(info.get_str("id"), Some("pl-native"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].get_str("id"), Some("v-second"));

    let page_extractor = GlomexExtractor::new(ExtractorDescriptor::new(
        "GlomexIE",
        "glomex",
        r#"https?://video\.glomex\.com/[^/]+/(?P<id>v-[^-]+)"#,
        true,
    ))
    .unwrap();
    let ExtractorResult::Redirect { url, ie_key } = page_extractor
        .extract_with_context("https://video.glomex.com/sport/v-native-title", &context)
        .unwrap()
    else {
        panic!("expected Glomex redirect");
    };
    assert_eq!(ie_key.as_deref(), Some("GlomexEmbed"));
    assert!(url.contains("playlistId=v-native"));
    assert!(url.contains("integrationId=19syy24xjn1oqlpc"));
}
