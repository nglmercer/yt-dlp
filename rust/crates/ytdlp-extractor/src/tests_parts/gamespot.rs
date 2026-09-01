#[test]
fn gamespot_native_extractor_maps_embedded_adaptive_streams() {
    let extractor = GameSpotExtractor::new(ExtractorDescriptor::new(
        "GameSpotIE",
        "GameSpot",
        r#"https?://(?:www\.)?gamespot\.com/(?:video|article|review)s/(?:[^/]+/\d+-|embed/)(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
            <meta name="description" content="Native GameSpot description">
            <meta property="og:image" content="https://cdn.example/gamespot.jpg">
        </head><body>
            <div data-video='{"guid":"gs-2300-6410818","title":"Arma%203%20Guide",
                "videoStreams":{"adaptive_stream":"https://cdn.example/master.m3u8",
                "adaptive_dash":"https://cdn.example/manifest.mpd"}}'></div>
        </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.gamespot.com/videos/arma-3-community-guide-sitrep-i/2300-6410818/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("gs-2300-6410818"));
    assert_eq!(result.get_str("display_id"), Some("6410818"));
    assert_eq!(result.get_str("title"), Some("Arma 3 Guide"));
    assert_eq!(
        result.get_str("description"),
        Some("Native GameSpot description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/gamespot.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/master.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("url"))
            .and_then(serde_json::Value::as_str),
        Some("https://cdn.example/master.mp4")
    );
}
