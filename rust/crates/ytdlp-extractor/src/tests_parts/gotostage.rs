#[test]
fn gotostage_native_extractor_registers_and_resolves_asset() {
    let extractor = GoToStageExtractor::new(ExtractorDescriptor::new(
        "GoToStageIE",
        "GoToStage",
        r#"https?://(?:www\.)?gotostage\.com/channel/[a-z0-9]+/recording/(?P<id>[a-z0-9]+)/watch"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api.gotostage.com/contents?ids=recording123".to_owned(),
                br#"[{
                    "product":"GoToStage",
                    "contentType":"recording",
                    "productRefKey":"ref-123",
                    "title":"Native GoToStage recording",
                    "duration":93.924711,
                    "category":"Education",
                    "thumbnail":{"location":"https://cdn.example/gotostage.jpg"}
                }]"#
                .to_vec(),
            ),
            (
                "api-registrations.logmeininc.com/registrations".to_owned(),
                br#"{"registrationKey":"registration-123"}"#.to_vec(),
            ),
            (
                "api.gotostage.com/contents/recording123/asset".to_owned(),
                br#"{"cdnLocation":"https://cdn.example/gotostage/recording.mp4"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.gotostage.com/channel/8901680603948959494/recording/recording123/watch",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("recording123"));
    assert_eq!(result.get_str("title"), Some("Native GoToStage recording"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/gotostage/recording.mp4")
    );
    assert_eq!(result.get_f64("duration"), Some(93.924711));
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["Education"]))
    );
}
