#[test]
fn godtube_native_extractor_maps_xml_metadata_and_media() {
    let extractor = GodTubeExtractor::new(ExtractorDescriptor::new(
        "GodTubeIE",
        "GodTube",
        r#"https?://(?:www\.)?godtube\.com/watch/\?v=(?P<id>[\da-zA-Z]+)"#,
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "godtube.com/resource/mediaplayer/0c0cnnnu.xml".to_owned(),
                br#"<config><file>https://cdn.example/godtube.mp4</file><author>beverlybmusic</author><date>2008-03-17</date><duration>2:39</duration><image>https://cdn.example/godtube.jpg</image></config>"#.to_vec(),
            ),
            (
                "godtube.com/media/xml/?v=0C0CNNNU".to_owned(),
                br#"<media><title>Woman at the well.</title></media>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.godtube.com/watch/?v=0C0CNNNU", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("0C0CNNNU"));
    assert_eq!(result.get_str("title"), Some("Woman at the well."));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/godtube.mp4"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get_str("uploader"), Some("beverlybmusic"));
    assert_eq!(result.get_i64("timestamp"), Some(1_205_712_000));
    assert_eq!(result.get_f64("duration"), Some(159.0));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/godtube.jpg")
    );
}
