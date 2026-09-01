fn mdr_extractor() -> MdrExtractor {
    MdrExtractor::new(ExtractorDescriptor::new(
        "MDRIE",
        "MDR",
        r#"https?://(?:www\.)?mdr\.de/(?:.*)/[a-z-]+-?(?P<id>\d+)(?:_.+?)?\.html"#,
        true,
    ))
    .unwrap()
}

#[test]
fn mdr_native_extractor_maps_xml_assets_and_broadcast_metadata() {
    let page = br#"<script>
        var dataURL: "https:\/\/cdn.example\/mdr\/native-avCustom.xml";
    </script>"#
    .to_vec();
    let xml = br#"<video>
        <title>Native MDR report</title>
        <type>video</type>
        <duration>04:10</duration>
        <rights>MITTELDEUTSCHER RUNDFUNK</rights>
        <broadcast>
            <broadcastDescription>Native MDR description</broadcastDescription>
            <broadcastDate>2026-08-31T12:00:00Z</broadcastDate>
        </broadcast>
        <assets>
            <asset>
                <progressiveDownloadUrl>https://cdn.example/mdr/native.mp4</progressiveDownloadUrl>
                <mediaType>MP4</mediaType>
                <bitrateVideo>1000000</bitrateVideo>
                <bitrateAudio>128000</bitrateAudio>
                <fileSize>42000000</fileSize>
                <frameWidth>1280</frameWidth>
                <frameHeight>720</frameHeight>
            </asset>
            <asset>
                <adaptiveHttpStreamingRedirectorUrl>https://cdn.example/mdr/native.m3u8</adaptiveHttpStreamingRedirectorUrl>
            </asset>
        </assets>
    </video>"#
    .to_vec();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            ("native-avCustom.xml".to_owned(), xml),
            ("mdr.de/kultur/video-native-1312272".to_owned(), page),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = mdr_extractor()
        .extract_with_context(
            "https://www.mdr.de/kultur/video-native-1312272.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1312272"));
    assert_eq!(result.get_str("title"), Some("Native MDR report"));
    assert_eq!(
        result.get_str("description"),
        Some("Native MDR description")
    );
    assert_eq!(
        result.get_str("uploader"),
        Some("MITTELDEUTSCHER RUNDFUNK")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(250.0)));
    assert!(result.get_i64("timestamp").is_some());
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let formats = result.get("formats").and_then(serde_json::Value::as_array).unwrap();
    assert_eq!(formats[0].get("vbr"), Some(&serde_json::json!(1000)));
    assert_eq!(formats[0].get("abr"), Some(&serde_json::json!(128)));
    assert_eq!(formats[0].get("width"), Some(&serde_json::json!(1280)));
    assert_eq!(formats[1].get("protocol"), Some(&serde_json::json!("m3u8_native")));
}

#[test]
fn mdr_native_extractor_marks_legacy_f4m_as_todo() {
    let page = br#"<script>var playerXml: 'https://cdn.example/mdr/native-avCustom.xml';</script>"#
        .to_vec();
    let xml = br#"<video>
        <title>Native MDR legacy stream</title>
        <assets><asset><downloadUrl>https://cdn.example/mdr/native.f4m</downloadUrl></asset></assets>
    </video>"#
    .to_vec();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            ("native-avCustom.xml".to_owned(), xml),
            ("mdr.de/kultur/video-native-100".to_owned(), page),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = mdr_extractor()
        .extract_with_context(
            "https://mdr.de/kultur/video-native-100.html",
            &context,
        )
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
