#[test]
fn elonet_native_extractor_maps_embedded_hls_source() {
    let extractor = ElonetExtractor::new(ExtractorDescriptor::new(
        "ElonetIE",
        "Elonet",
        r"https?://elonet\.finna\.fi/Record/kavi\.elonet_elokuva_(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "elonet.finna.fi/Record/kavi.elonet_elokuva_107867".to_owned(),
            br#"<html><head>
                    <meta property="og:title" content="Valkoinen peura">
                    <meta property="og:description" content="A native description">
                    <meta property="og:image" content="https://img.example/elonet/107867.jpg">
                </head><body>
                    <div id='video-data' data-video-sources="[{&quot;src&quot;:&quot;https://cdn.example/elonet/107867/master.m3u8&quot;}]"></div>
                </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://elonet.finna.fi/Record/kavi.elonet_elokuva_107867",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("107867"));
    assert_eq!(result.get_str("title"), Some("Valkoinen peura"));
    assert_eq!(result.get_str("description"), Some("A native description"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/elonet/107867/master.m3u8")
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
}

#[test]
fn elonet_native_extractor_marks_unknown_streams_as_todo() {
    let extractor = ElonetExtractor::new(ExtractorDescriptor::new(
        "ElonetIE",
        "Elonet",
        r"https?://elonet\.finna\.fi/Record/kavi\.elonet_elokuva_(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "elonet.finna.fi/Record/kavi.elonet_elokuva_1".to_owned(),
            br#"<div id='video-data' data-video-sources="[{&quot;src&quot;:&quot;https://cdn.example/elonet/1/stream.ism&quot;}]"></div>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://elonet.finna.fi/Record/kavi.elonet_elokuva_1",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
