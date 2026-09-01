#[test]
fn expressen_native_extractor_maps_embedded_json_and_hls_metadata() {
    let extractor = ExpressenExtractor::new(ExtractorDescriptor::new(
        "ExpressenIE",
        "Expressen",
        r#"(?x)
            https?://
                (?:www\.)?(?:expressen|di)\.se/
                (?:(?:tvspelare/video|video-?player/embed)/)?
                (?:tv|nyheter)/(?:[^/?#]+/)*
                (?P<id>[^/?#&]+)
        "#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html>
            <div data-video-tracking-info='{"contentId":"native-content","titleRaw":"Native Expressen","descriptionRaw":"Native description","socialMediaImage":"https://cdn.example/native.jpg","videoTotalSecondsDuration":788,"publishDate":"2018-05-18T12:00:00Z"}'></div>
            <div data-article-data='{"stream":"https://cdn.example/native.m3u8","title":"Article title","image":"https://cdn.example/article.jpg"}'></div>
        </html>"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.expressen.se/tv/ledare/native-expressen/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-content"));
    assert_eq!(result.get_str("display_id"), Some("native-expressen"));
    assert_eq!(result.get_str("title"), Some("Native Expressen"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/native.jpg")
    );
    assert_eq!(result.get_i64("duration"), Some(788));
    assert!(result.get_i64("timestamp").is_some());
    assert_eq!(result.get_str("ext"), Some("mp4"));
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
fn expressen_native_extractor_falls_back_to_article_direct_stream() {
    let extractor = ExpressenExtractor::new(ExtractorDescriptor::new(
        "ExpressenIE",
        "Expressen",
        r"https?://(?:www\.)?(?:expressen|di)\.se/(?:tv|nyheter)/(?P<id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div data-video-tracking-info='{"contentId":123,"titleRaw":""}'></div>
            <div data-article-data='{"stream":"https://cdn.example/native.mp4","title":"Article fallback"}'></div>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://di.se/tv/native-direct",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("123"));
    assert_eq!(result.get_str("title"), Some("Article fallback"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/native.mp4"));
}
