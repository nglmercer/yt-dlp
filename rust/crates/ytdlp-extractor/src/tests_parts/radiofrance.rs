#[test]
fn radiofrance_native_extractor_maps_radiovisions_audio_sources() {
    let extractor = RadioFranceExtractor::new(ExtractorDescriptor::new(
        "RadioFranceIE",
        "radiofrance",
        r#"https?://maison\.radiofrance\.fr/radiovisions/(?P<id>[^?#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: r#"<html><body>
            <h1>One to one</h1>
            <div class="bloc_page_wrapper"><div class="text">
                Page description with <strong>formatting</strong>.
            </div></div>
            <div class="credit">&nbsp;&nbsp;&copy;&nbsp;Thomas Hercouët</div>
            <div class="jp-jplayer" data-source="ogg: 'https://media.example/one.ogg', mp3: 'https://media.example/one.mp3'"></div>
        </body></html>"#
            .as_bytes()
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://maison.radiofrance.fr/radiovisions/one-one",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("one-one"));
    assert_eq!(result.get_str("title"), Some("One to one"));
    assert_eq!(
        result.get_str("description"),
        Some("Page description with formatting.")
    );
    assert_eq!(result.get_str("uploader"), Some("Thomas Hercouët"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("ogg")));
    assert_eq!(
        formats[0].get("url"),
        Some(&serde_json::json!("https://media.example/one.ogg"))
    );
    assert_eq!(formats[0].get("quality"), Some(&serde_json::json!(0)));
    assert_eq!(formats[1].get("ext"), Some(&serde_json::json!("mp3")));
    assert_eq!(formats[1].get("vcodec"), Some(&serde_json::json!("none")));
}

#[test]
fn radiofrance_native_extractor_requires_audio_sources() {
    let extractor = RadioFranceExtractor::new(ExtractorDescriptor::new(
        "RadioFranceIE",
        "radiofrance",
        r#"https?://maison\.radiofrance\.fr/radiovisions/(?P<id>[^?#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<h1>Missing sources</h1>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://maison.radiofrance.fr/radiovisions/missing",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("has no audio URLs"));
}
