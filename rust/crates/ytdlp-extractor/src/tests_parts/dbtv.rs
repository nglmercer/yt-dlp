#[test]
fn dbtv_native_extractor_routes_jwplatform_video_ids_transparently() {
    let extractor = DbtvExtractor::new(ExtractorDescriptor::new(
        "DBTVIE",
        "DBTV",
        r"(?x)
        https?://(?:www\.)?dagbladet\.no/video/
        (?:(?:embed|(?P<display_id>[^/]+))/)?
        (?P<id>[0-9A-Za-z_-]{11}|[a-zA-Z0-9]{8})",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.dagbladet.no/video/truer-iran-bor-passe-dere/Ab12Cd34",
            &ExtractionContext::native(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(result.get_str("id"), Some("Ab12Cd34"));
    assert_eq!(result.get_str("display_id"), Some("truer-iran-bor-passe-dere"));
    assert_eq!(result.get_str("url"), Some("jwplatform:Ab12Cd34"));
    assert_eq!(result.get_str("ie_key"), Some("JWPlatform"));
}

#[test]
fn dbtv_native_extractor_routes_youtube_video_ids_transparently() {
    let extractor = DbtvExtractor::new(ExtractorDescriptor::new(
        "DBTVIE",
        "DBTV",
        r"(?x)
        https?://(?:www\.)?dagbladet\.no/video/
        (?:(?:embed|(?P<display_id>[^/]+))/)?
        (?P<id>[0-9A-Za-z_-]{11}|[a-zA-Z0-9]{8})",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.dagbladet.no/video/embed/PynxJnNWChE/",
            &ExtractionContext::native(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("PynxJnNWChE"));
    assert_eq!(result.get_str("url"), Some("PynxJnNWChE"));
    assert_eq!(result.get_str("ie_key"), Some("Youtube"));
}
