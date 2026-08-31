#[test]
fn clipchamp_native_extractor_maps_cloudflare_stream_manifests() {
    let extractor = ClipchampExtractor::new(ExtractorDescriptor::new(
        "ClipchampIE",
        "Clipchamp",
        r"https?://(?:www\.)?clipchamp\.com/watch/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "iframe.cloudflarestream.com/native-path".to_owned(),
                br#"<div customer-domain-prefix="customer-native"></div>"#.to_vec(),
            ),
            (
                "clipchamp.com/watch/gRXZ4ZhdDaU".to_owned(),
                br#"<html><script id="__NEXT_DATA__" type="application/json">{
                    "props":{"pageProps":{"video":{
                        "storage_location":"cf_stream",
                        "download_url":"native-path",
                        "project":{"project_name":"Native Clipchamp video"},
                        "creator":{"first_name":"Alexander","last_name":"Schwartz"},
                        "created_at":"2023-04-06T12:26:20Z",
                        "thumbnail_url":"https://cdn.example/clipchamp/poster.jpg"
                    }}}
                }</script></html>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://clipchamp.com/watch/gRXZ4ZhdDaU", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("gRXZ4ZhdDaU"));
    assert_eq!(result.get_str("title"), Some("Native Clipchamp video"));
    assert_eq!(result.get_str("uploader"), Some("Alexander Schwartz"));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1680783980i64))
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/clipchamp/poster.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert!(formats.iter().any(|format| {
        format.get("protocol") == Some(&serde_json::json!("http_dash_segments"))
            && format
                .get("url")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|url| url.contains("customer-native.cloudflarestream.com"))
    }));
    assert!(formats.iter().any(|format| {
        format.get("protocol") == Some(&serde_json::json!("m3u8_native"))
    }));
}

#[test]
fn clipchamp_native_extractor_marks_non_cloudflare_storage_as_todo() {
    let extractor = ClipchampExtractor::new(ExtractorDescriptor::new(
        "ClipchampIE",
        "Clipchamp",
        r"https?://clipchamp\.com/watch/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script id="__NEXT_DATA__">{"props":{"pageProps":{"video":{"storage_location":"s3"}}}}</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://clipchamp.com/watch/native", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
