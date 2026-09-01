#[test]
fn dfb_native_extractor_maps_tokenized_hls_manifests_and_metadata() {
    let extractor = DfbExtractor::new(ExtractorDescriptor::new(
        "DFBIE",
        "tv.dfb.de",
        r"https?://tv\.dfb\.de/video/(?P<display_id>[^/]+)/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "tv.dfb.de/server/hd_video.php?play=11633".to_owned(),
                r#"<video><url>//access.example/stream?token=native</url>
                    <title>Native DFB video</title><time_date>14.07.2015</time_date></video>"#
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "access.example/stream?token=native&area=&format=iphone".to_owned(),
                br#"<stream><token url="https://cdn.example/dfb/iphone.m3u8" auth="iphone-token"/></stream>"#
                    .to_vec(),
            ),
            (
                "access.example/stream?token=native".to_owned(),
                br#"<stream><token url="https://cdn.example/dfb/native.m3u8" auth="native-token"/></stream>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://tv.dfb.de/video/u-19-em-stimmen-zum-spiel-gegen-russland/11633/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("11633"));
    assert_eq!(
        result.get_str("display_id"),
        Some("u-19-em-stimmen-zum-spiel-gegen-russland")
    );
    assert_eq!(result.get_str("title"), Some("Native DFB video"));
    assert_eq!(result.get_str("upload_date"), Some("20150714"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/dfb/native.m3u8?hdnea=native-token")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn dfb_native_extractor_marks_hds_manifests_as_todo() {
    let extractor = DfbExtractor::new(ExtractorDescriptor::new(
        "DFBIE",
        "tv.dfb.de",
        r"https?://tv\.dfb\.de/video/(?P<display_id>[^/]+)/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "tv.dfb.de/server/hd_video.php?play=11633".to_owned(),
                br#"<video><url>https://access.example/stream</url></video>"#.to_vec(),
            ),
            (
                "access.example/stream".to_owned(),
                br#"<stream><token url="https://cdn.example/dfb/native.f4m" auth="native-token"/></stream>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://tv.dfb.de/video/native/11633/",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
