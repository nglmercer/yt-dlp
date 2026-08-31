#[test]
fn golem_native_extractor_maps_xml_formats_and_thumbnails() {
    let extractor = GolemExtractor::new(ExtractorDescriptor::new(
        "GolemIE",
        "Golem",
        r"https?://video\.golem\.de/.+?/(?P<id>.+?)/",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "video.golem.de/xml/14095.xml".to_owned(),
            br#"<config>
                <title>iPhone 6 &amp; 6 Plus - Test</title>
                <playtime>300.44</playtime>
                <high width="1280" height="720">
                    <url>/media/14095_high.mp4</url>
                    <filename>high.mp4</filename>
                    <filesize>65309548</filesize>
                </high>
                <low width="640" height="360">
                    <url>/media/14095_low.mp4</url>
                    <filename>low.mp4</filename>
                </low>
                <teaser width="320" height="180"><url>/images/teaser.jpg</url></teaser>
            </config>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://video.golem.de/handy/14095/iphone-6-und-6-plus-test.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("14095"));
    assert_eq!(result.get_str("title"), Some("iPhone 6 & 6 Plus - Test"));
    assert_eq!(result.get_f64("duration"), Some(300.44));
    assert_eq!(
        result.get_str("url"),
        Some("http://video.golem.de/media/14095_high.mp4")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("high")));
    assert_eq!(formats[0].get("width"), Some(&serde_json::json!(1280)));
    assert_eq!(formats[0].get("height"), Some(&serde_json::json!(720)));
    assert_eq!(
        result.get("thumbnails"),
        Some(&serde_json::json!([{
            "url":"http://video.golem.de/images/teaser.jpg",
            "width":320,
            "height":180
        }]))
    );
}
