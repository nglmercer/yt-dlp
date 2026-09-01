#[test]
fn faz_native_extractor_maps_embedded_encoding_xml() {
    let extractor = FazExtractor::new(ExtractorDescriptor::new(
        "FazIE",
        "faz.net",
        r"https?://(?:www\.)?faz\.net/(?:[^/]+/)*.*?-(?P<id>\d+)\.html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html>
            <meta property="og:title" content="Native FAZ title">
            <meta property="og:description" content="A &amp; useful description">
            <div data-videojs-media='
                <PLAYER>
                    <ENCODINGS>
                        <LOW><FILENAME>https://cdn.example/640x360_400.mp4</FILENAME><AVERAGEBITRATE>400</AVERAGEBITRATE><CODEC>h264</CODEC></LOW>
                        <HIGH><FILENAME>https://cdn.example/1280x720_1200.mp4</FILENAME><AVERAGEBITRATE>1200</AVERAGEBITRATE><CODEC>h264</CODEC></HIGH>
                        <HQ><FILENAME>https://cdn.example/1920x1080_2400.mp4</FILENAME><CODEC>h264</CODEC></HQ>
                    </ENCODINGS>
                    <STILL><STILL_BIG>https://cdn.example/poster.jpg</STILL_BIG></STILL>
                    <DURATION>265</DURATION>
                </PLAYER>'></div>
        </html>"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.faz.net/multimedia/videos/native-title-12610585.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("12610585"));
    assert_eq!(result.get_str("title"), Some("Native FAZ title"));
    assert_eq!(
        result.get_str("description"),
        Some("A & useful description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/poster.jpg")
    );
    assert_eq!(result.get_i64("duration"), Some(265));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.iter().find(|format| {
                format.get("format_id") == Some(&serde_json::json!("high"))
            }))
            .and_then(|format| format.get("height")),
        Some(&serde_json::json!(720))
    );
}

#[test]
fn faz_native_extractor_redirects_external_perform_player() {
    let extractor = FazExtractor::new(ExtractorDescriptor::new(
        "FazIE",
        "faz.net",
        r"https?://(?:www\.)?faz\.net/(?:[^/]+/)*.*?-(?P<id>\d+)\.html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div data-videojs-media='extern'></div>
            <iframe src="//player.performgroup.com/eplayer/eplayer.html#/0123456789abcdef0123456789.0123456789abcdef0123456789"></iframe>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        extractor
            .extract_with_context(
                "https://www.faz.net/aktuell/native-13659345.html",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "http://player.performgroup.com/eplayer/eplayer.html#/0123456789abcdef0123456789.0123456789abcdef0123456789".to_owned(),
            ie_key: None,
        }
    );
}
