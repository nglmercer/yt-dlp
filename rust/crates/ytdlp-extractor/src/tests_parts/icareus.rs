#[test]
fn icareus_native_extractor_maps_playback_urls_subtitles_and_metadata() {
    let extractor = IcareusExtractor::new(ExtractorDescriptor::new(
        "IcareusIE",
        "Icareus",
        r#"(?P<base_url>https?://(?:www\.)?(?:asahitv\.fi|helsinkikanava\.fi|hyvinvointitv\.fi|inez\.fi|permanto\.fi|suite\.icareus\.com|videos\.minifiddlers\.org))/[^?#]+/player/[^?#]+\?(?:[^#]+&)?(?:assetId|eventId)=(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "helsinkikanava.fi/fi/web/helsinkikanava/player/vod".to_owned(),
                br#"<html><head>
                    <script>
                        _icareus['itemId'] = '68021894';
                        _icareus['organizationId'] = 'native-org';
                        _icareus['token'] = 'abcdef';
                        var publishingServiceURL = "https://api.icareus.example/playback";
                    </script>
                    <script type="application/ld+json">{
                        "title":"Native Icareus title",
                        "description":"Native Icareus description",
                        "datePublished":"2020-09-24T10:00:00Z",
                        "thumbnail":"https://cdn.example/icareus.jpg"
                    }</script>
                </head></html>"#
                    .to_vec(),
            ),
            (
                "api.icareus.example/playback".to_owned(),
                br#"{
                    "thumbnail":"https://cdn.example/fallback.jpg",
                    "subtitles":[["en","en: English","https://cdn.example/native.vtt"]],
                    "audio_urls":[{"name":"128 kbps","url":"https://cdn.example/native.mp3"}],
                    "urls":[
                        {"id":"hls","name":"1280x720 2500 kbps","url":"https://cdn.example/native.m3u8"},
                        {"id":"mp4","name":"640x360","url":"https://cdn.example/native.mp4"}
                    ]
                }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.helsinkikanava.fi/fi/web/helsinkikanava/player/vod?assetId=68021894",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("68021894"));
    assert_eq!(result.get_str("title"), Some("Native Icareus title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Icareus description")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/native.mp3")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("vcodec"), Some(&serde_json::json!("none")));
    assert_eq!(
        formats[1].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!("https://cdn.example/native.vtt"))
    );
}
