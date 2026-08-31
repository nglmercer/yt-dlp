#[test]
fn angel_native_extractor_maps_jsonld_hls_and_metadata() {
    let extractor = AngelExtractor::new(ExtractorDescriptor::new(
        "AngelIE",
        "Angel",
        r#"https?://(?:www\.)?angel\.com/watch/(?P<series>[^/?#]+)/episode/(?P<id>[\w-]+)/season-(?P<season_number>\d+)/episode-(?P<episode_number>\d+)/(?P<title>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "angel.com/watch/tuttle-twins/episode/2f3d0382-ea82-4cdc-958e-84fbadadc710"
                .to_owned(),
            br#"<html><head>
                    <meta property="og:title" content="Tuttle Twins Season 1, Episode 1">
                    <meta property="og:description" content="Native &amp; precise episode description">
                    <meta property="og:image" content="https://images.angelstudios.com/image/upload/v123/w_720/angel-app/episode.jpg">
                </head><body>
                    <script type="application/ld+json">{
                        "@context": "https://schema.org",
                        "@type": "VideoObject",
                        "contentUrl": "https://cdn.example/angel/episode.m3u8",
                        "name": "JSON-LD fallback title",
                        "description": "JSON-LD fallback description",
                        "thumbnailUrl": "https://images.angelstudios.com/image/upload/angel-app/fallback.jpg",
                        "duration": "PT22M39S",
                        "uploadDate": "2023-01-02T03:04:05Z",
                        "author": {"@type": "Organization", "name": "Angel Studios"},
                        "width": 1280,
                        "height": 720,
                        "keywords": "family,education"
                    }</script>
                </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.angel.com/watch/tuttle-twins/episode/2f3d0382-ea82-4cdc-958e-84fbadadc710/season-1/episode-1/when-laws-give-you-lemons",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("2f3d0382-ea82-4cdc-958e-84fbadadc710")
    );
    assert_eq!(
        result.get_str("title"),
        Some("Tuttle Twins Season 1, Episode 1")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native & precise episode description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://images.angelstudios.com/image/upload/angel-app/episode.jpg")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(1359.0)));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1672628645i64))
    );
    assert_eq!(result.get_str("uploader"), Some("Angel Studios"));
    assert_eq!(result.get("tags"), Some(&serde_json::json!(["family", "education"])));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/angel/episode.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("m3u8_native")
    );
    assert_eq!(result.get("subtitles"), Some(&serde_json::json!({})));
}

#[test]
fn angel_native_extractor_marks_non_hls_jsonld_stream_as_todo() {
    let extractor = AngelExtractor::new(ExtractorDescriptor::new(
        "AngelIE",
        "Angel",
        r#"https?://angel\.com/watch/(?P<series>[^/?#]+)/episode/(?P<id>[\w-]+)/season-(?P<season_number>\d+)/episode-(?P<episode_number>\d+)/(?P<title>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script type="application/ld+json">{"contentUrl":"https://cdn.example/angel/episode.mp4"}</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://angel.com/watch/show/episode/episode-id/season-1/episode-1/title",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
