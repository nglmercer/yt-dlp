fn meipai_extractor() -> MeipaiExtractor {
    MeipaiExtractor::new(ExtractorDescriptor::new(
        "MeipaiIE",
        "Meipai",
        r#"https?://(?:www\.)?meipai\.com/media/(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap()
}

#[test]
fn meipai_native_extractor_maps_recorded_hls_and_open_graph_metadata() {
    let body = r###"<html>
        <meta property="og:title" content="#葉子##阿桑##余姿昀##超級女聲#">
        <meta property="og:description" content="#葉子##阿桑##余姿昀##超級女聲#">
        <meta property="og:image" content="https://cdn.example/meipai/native.jpg">
        <meta property="video:release_date" content="2016-06-09T00:00:00Z">
        <meta property="video:tag" content="葉子,阿桑,余姿昀,超級女聲">
        <meta name="interactionCount" content="35511">
        <meta name="duration" content="152">
        <meta property="video:director" content="她她-TATA">
        <script>file: encodeURIComponent("https://cdn.example/meipai/native.m3u8")</script>
    </html>"###
    .as_bytes()
    .to_vec();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler { body });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = meipai_extractor()
        .extract_with_context("http://www.meipai.com/media/531697625", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("531697625"));
    assert_eq!(
        result.get_str("title"),
        Some("#葉子##阿桑##余姿昀##超級女聲#")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(152.0)));
    assert_eq!(result.get_i64("view_count"), Some(35511));
    assert!(result.get_i64("timestamp").is_some());
    assert_eq!(result.get_str("creator"), Some("她她-TATA"));
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["葉子", "阿桑", "余姿昀", "超級女聲"]))
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/meipai/native.m3u8")
    );
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
fn meipai_native_extractor_falls_back_to_direct_video_markup() {
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div data-video="https://cdn.example/meipai/native.mp4"></div>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = meipai_extractor()
        .extract_with_context("https://www.meipai.com/media/585526361", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/meipai/native.mp4")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("format_id")),
        Some(&serde_json::json!("http"))
    );
}
