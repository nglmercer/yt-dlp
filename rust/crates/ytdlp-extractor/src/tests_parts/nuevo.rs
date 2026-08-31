#[test]
fn nonktube_native_extractor_maps_html5_media_and_metadata() {
    let extractor = NonkTubeExtractor::new(ExtractorDescriptor::new(
        "NonkTubeIE",
        "NonkTube",
        r#"https?://(?:www\.)?nonktube\.com/(?:(?:video|embed)/|media/nuevo/embed\.php\?.*?\bid=)(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.nonktube.com/video/118636".to_owned(),
            br#"<html><head>
                    <meta property="og:title" content="Native NonkTube title">
                    <meta property="og:video:duration" content="1150.98">
                </head><body>
                    <video poster="/covers/118636.jpg">
                        <source src="/media/118636.mp4" type="video/mp4">
                    </video>
                </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.nonktube.com/video/118636/sensual-title",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("118636"));
    assert_eq!(result.get_str("title"), Some("Native NonkTube title"));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(18)));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(1150.98)));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://www.nonktube.com/covers/118636.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://www.nonktube.com/media/118636.mp4")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("http")
    );
}

#[test]
fn lovehomeporn_native_extractor_maps_nuevo_xml_config() {
    let extractor = LoveHomePornExtractor::new(ExtractorDescriptor::new(
        "LoveHomePornIE",
        "LoveHomePorn",
        r#"https?://(?:www\.)?lovehomeporn\.com/video/(?P<id>\d+)(?:/(?P<display_id>[^/?#&]+))?"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "lovehomeporn.com/media/nuevo/config.php?key=48483".to_owned(),
            br#"<config>
                    <title>Native LoveHomePorn title</title>
                    <mediaid>48483</mediaid>
                    <image>https://cdn.example/lovehomeporn/48483.jpg</image>
                    <duration>238.47</duration>
                    <file>https://cdn.example/lovehomeporn/48483.mp4</file>
                    <filehd>https://cdn.example/lovehomeporn/48483-hd.mp4</filehd>
                </config>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://lovehomeporn.com/video/48483/stunning-title",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("48483"));
    assert_eq!(result.get_str("display_id"), Some("stunning-title"));
    assert_eq!(result.get_str("title"), Some("Native LoveHomePorn title"));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(18)));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(238.47)));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/lovehomeporn/48483.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("sd")));
    assert_eq!(formats[1].get("format_id"), Some(&serde_json::json!("hd")));
}
