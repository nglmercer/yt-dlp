#[test]
fn kth_native_extractor_uses_kaltura_service_configuration() {
    let extractor = KthExtractor::new(ExtractorDescriptor::new(
        "KTHIE",
        "KTH",
        r#"https?://play\.kth\.se/(?:[^/]+/)+(?P<id>[a-z0-9_]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://play.kth.se/media/native-title/0_uoop6oz9",
            &kaltura_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("0_uoop6oz9"));
    assert_eq!(result.get_str("title"), Some("Native Kaltura title"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/serveFlavor/flavorId/video-1")
    );
}
