#[test]
fn cloudflare_stream_native_extractor_maps_signed_manifest_urls() {
    let extractor = CloudflareStreamExtractor::new(ExtractorDescriptor::new(
        "CloudflareStreamIE",
        "CloudflareStream",
        r"https?://(?:(?:(?:watch|iframe|customer-[\w-]+)\.)?(?P<domain>(?:cloudflarestream\.com|(?:videodelivery|bytehighway)\.net))/|(?:embed\.|(?:(?:watch|iframe|customer-[\w-]+)\.)?(?:cloudflarestream\.com|(?:videodelivery|bytehighway)\.net)/embed/[^/?#]+\.js\?(?:[^#]+&)?video=))(?P<id>[\da-f]{32}|eyJ[\w-]+\.[\w-]+\.[\w-]+)",
        true,
    ))
    .unwrap();
    let url = "https://watch.cloudflarestream.com/eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJuYXRpdmUtaWQifQ.signature";
    let result = extractor
        .extract_with_context(url, &ExtractionContext::native())
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-id"));
    assert_eq!(result.get_str("title"), Some("native-id"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cloudflarestream.com/eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJuYXRpdmUtaWQifQ.signature/manifest/video.m3u8")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cloudflarestream.com/eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJuYXRpdmUtaWQifQ.signature/thumbnails/thumbnail.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("hls")));
    assert_eq!(formats[1].get("format_id"), Some(&serde_json::json!("dash")));
}

#[test]
fn cloudflare_stream_native_extractor_marks_invalid_signed_ids_as_todo() {
    let extractor = CloudflareStreamExtractor::new(ExtractorDescriptor::new(
        "CloudflareStreamIE",
        "CloudflareStream",
        r"https?://(?:(?:(?:watch|iframe|customer-[\w-]+)\.)?(?P<domain>(?:cloudflarestream\.com|(?:videodelivery|bytehighway)\.net))/|(?:embed\.|(?:(?:watch|iframe|customer-[\w-]+)\.)?(?:cloudflarestream\.com|(?:videodelivery|bytehighway)\.net)/embed/[^/?#]+\.js\?(?:[^#]+&)?video=))(?P<id>[\da-f]{32}|eyJ[\w-]+\.[\w-]+\.[\w-]+)",
        true,
    ))
    .unwrap();
    let error = extractor
        .extract_with_context(
            "https://watch.cloudflarestream.com/eyJheader.invalid-payload.signature",
            &ExtractionContext::native(),
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
