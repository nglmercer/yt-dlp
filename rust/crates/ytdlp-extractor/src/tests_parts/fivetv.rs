#[test]
fn fivetv_native_extractor_maps_html5_player_metadata() {
    let extractor = FiveTvExtractor::new(ExtractorDescriptor::new(
        "FiveTVIE",
        "FiveTV",
        r"(?x)
            https?://
                (?:www\.)?5-tv\.ru/
                (?:
                    (?:[^/]+/)+(?P<id>\d+)|
                    (?P<path>[^/?#]+)(?:[/?#])?
                )
        ",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
            <title>Native Five TV story</title>
            <meta property="og:description" content="Native description">
            <meta property="og:image" content="https://cdn.example/five.jpg">
            <meta property="video:duration" content="180">
        </head><body>
            <div class="flowplayer" data-href="https://cdn.example/five/master.m3u8"></div>
        </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.5-tv.ru/news/96814/", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("96814"));
    assert_eq!(result.get_str("title"), Some("Native Five TV story"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/five.jpg")
    );
    assert_eq!(result.get_i64("duration"), Some(180));
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
fn fivetv_native_extractor_supports_videoplayer_link() {
    let extractor = FiveTvExtractor::new(ExtractorDescriptor::new(
        "FiveTVIE",
        "FiveTV",
        r"(?x)
            https?://
                (?:www\.)?5-tv\.ru/
                (?:
                    (?:[^/]+/)+(?P<id>\d+)|
                    (?P<path>[^/?#]+)(?:[/?#])?
                )
        ",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<title>Five TV direct</title>
            <a href="/media/1021729.mp4" class="videoplayer">watch</a>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://5-tv.ru/video/1021729/", &context)
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("1021729"));
    assert_eq!(
        result.get_str("url"),
        Some("http://5-tv.ru/media/1021729.mp4")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}
