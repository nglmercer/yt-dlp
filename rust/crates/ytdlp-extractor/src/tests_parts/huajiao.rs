#[test]
fn huajiao_native_extractor_maps_feed_hls_metadata() {
    let extractor = HuajiaoExtractor::new(ExtractorDescriptor::new(
        "HuajiaoIE",
        "Huajiao",
        r#"https?://(?:www\.)?huajiao\.com/l/(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "huajiao.com/l/38941232".to_owned(),
            br#"<html><meta name="description" content="Native Huajiao description"><script>var feed = {"feed":{"formated_title":"Native Huajiao live","duration":"00:40:24","image":"https://cdn.example/huajiao.jpg","m3u8":"https://cdn.example/huajiao.m3u8"},"creatime":"2016-10-07 18:54:19","author":{"nickname":"Penny","uid":75206005}};</script></html>"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.huajiao.com/l/38941232", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("38941232"));
    assert_eq!(result.get_str("title"), Some("Native Huajiao live"));
    assert_eq!(result.get_f64("duration"), Some(2424.0));
    assert_eq!(result.get_i64("timestamp"), Some(1_475_866_459));
    assert_eq!(result.get_str("uploader"), Some("Penny"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/huajiao.m3u8"));
}
