#[test]
fn magenta_musik_native_extractor_maps_api_metadata_and_smil_media() {
    let extractor = MagentaMusikExtractor::new(ExtractorDescriptor::new(
        "MagentaMusikIE",
        "MagentaMusik",
        r#"https?://(?:www\.)?magentamusik\.de/(?P<id>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "magentamusik.de/marty-friedman".to_owned(),
                br#"<div data-js-element="o-video-player__config">{"assetId":"asset-1"}</div>"#
                    .to_vec(),
            ),
            (
                "/assetdetails/58938/asset-1".to_owned(),
                br#"{"content":{"partnerInformation":[{"reference":"video-1"}]}}"#.to_vec(),
            ),
            (
                "/player/58935/video-1/Main%20Movie".to_owned(),
                br#"{"content":{"feature":{"representations":[{"contentPackages":[{"media":{"href":"https://cdn.example/magenta.smil"}}]}],"metadata":{"title":"Marty Friedman: W:O:A 2023","originalTitle":"Konzert vom: 05.08.2023 13:00","longDescription":"Native concert description","runtimeInSeconds":"2760","countriesOfProduction":["Deutschland"],"yearOfProduction":"2023","mainGenre":"Musikkonzert"}}}}"#.to_vec(),
            ),
            (
                "cdn.example/magenta.smil".to_owned(),
                br#"<smil><head><meta name="base" content="https://cdn.example/media/"/></head><body>
                    <video src="concert.mp4" system-bitrate="1500000" width="1280" height="720"/>
                    <audio src="concert-audio.mp3" system-bitrate="128000"/>
                </body></smil>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.magentamusik.de/marty-friedman-woa-2023-9208205928595409235",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("video-1"));
    assert_eq!(
        result.get_str("display_id"),
        Some("marty-friedman-woa-2023-9208205928595409235")
    );
    assert_eq!(result.get_str("title"), Some("Marty Friedman: W:O:A 2023"));
    assert_eq!(
        result.get_str("alt_title"),
        Some("Konzert vom: 05.08.2023 13:00")
    );
    assert_eq!(result.get_i64("duration"), Some(2760));
    assert_eq!(result.get_str("location"), Some("Deutschland"));
    assert_eq!(result.get_i64("release_year"), Some(2023));
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["Musikkonzert"]))
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/media/concert.mp4")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn magenta_musik_native_extractor_fails_without_player_config() {
    let extractor = MagentaMusikExtractor::new(ExtractorDescriptor::new(
        "MagentaMusikIE",
        "MagentaMusik",
        r#"https?://(?:www\.)?magentamusik\.de/(?P<id>[^/?#]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: b"<html>no video</html>".to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.magentamusik.de/no-player", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no video player"));
}
