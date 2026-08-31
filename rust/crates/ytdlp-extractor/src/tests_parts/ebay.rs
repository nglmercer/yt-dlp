#[test]
fn ebay_native_extractor_maps_embedded_hls_and_dash_manifests() {
    let extractor = EbayExtractor::new(ExtractorDescriptor::new(
        "EbayIE",
        "Ebay",
        r"https?://(?:www\.)?ebay\.com/itm/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.ebay.com/itm/194509326719".to_owned(),
            br#"<html><head><title>Native item | eBay</title></head><body>
                <script>window.item = {"video":{"playlistMap":{
                    "HLS":"https://cdn.example/item/master.m3u8",
                    "DASH":"https://cdn.example/item/manifest.mpd",
                    "OTHER":"https://cdn.example/item/ignored"
                }}}</script>
            </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.ebay.com/itm/194509326719", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("194509326719"));
    assert_eq!(result.get_str("title"), Some("Native item"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/item/master.m3u8")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert!(formats.iter().any(|format| {
        format.get("format_id") == Some(&serde_json::json!("hls"))
            && format.get("protocol") == Some(&serde_json::json!("m3u8_native"))
    }));
    assert!(formats.iter().any(|format| {
        format.get("format_id") == Some(&serde_json::json!("dash"))
            && format.get("protocol") == Some(&serde_json::json!("http_dash_segments"))
    }));
}
