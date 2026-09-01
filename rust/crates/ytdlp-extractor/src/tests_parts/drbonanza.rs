#[test]
fn drbonanza_native_extractor_maps_html5_sources_and_asset_metadata() {
    let extractor = DrBonanzaExtractor::new(ExtractorDescriptor::new(
        "DRBonanzaIE",
        "DRBonanza",
        r"https?://(?:www\.)?dr\.dk/bonanza/[^/]+/\d+/[^/]+/(?P<id>\d+)/(?P<display_id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><video>
            <source src="/media/native.m3u8" type="application/vnd.apple.mpegurl">
            <source src="https://cdn.example/native.mp4" type="video/mp4">
        </video><script>
            currentAsset = {AssetId: 'native-asset', AssetTitle: 'Native Bonanza',
                AssetImageUrl: 'https://cdn.example/poster.jpg'}
        </script>
        <div class="label"><p>Programinfo:<p></div><div class="value"><p>Native description</p></div>
        <div class="label"><p>Tid:<p></div><div class="value"><p>01:02:03</p></div>
        </html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.dr.dk/bonanza/serie/154/matador/40312/native-bonanza-",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-asset"));
    assert_eq!(result.get_str("display_id"), Some("native-bonanza-"));
    assert_eq!(result.get_str("title"), Some("Native Bonanza"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(result.get_f64("duration"), Some(3_723.0));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/poster.jpg"));
    assert_eq!(
        result.get_str("url"),
        Some("https://www.dr.dk/media/native.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
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
