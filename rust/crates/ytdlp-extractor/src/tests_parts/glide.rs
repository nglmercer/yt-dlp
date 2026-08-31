#[test]
fn glide_native_extractor_maps_html5_source_and_thumbnail() {
    let extractor = GlideExtractor::new(ExtractorDescriptor::new(
        "GlideIE",
        "Glide",
        r"https?://share\.glide\.me/(?P<id>[A-Za-z0-9\-=_+]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "share.glide.me/UZF8zlmuQbe4mr+7dCiQ0w==".to_owned(),
            br#"<html><head><title>Damon's Glide message</title></head><body>
                <video><source src="//cdn.example/video.mp4?token=one&amp;part=two"></video>
                <img id="video-thumbnail" src="//cdn.example/thumb.jpg">
            </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://share.glide.me/UZF8zlmuQbe4mr+7dCiQ0w==",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("UZF8zlmuQbe4mr+7dCiQ0w=="));
    assert_eq!(result.get_str("title"), Some("Damon's Glide message"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/video.mp4?token=one&part=two")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/thumb.jpg")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}
