#[test]
fn eltrecetv_native_extractor_maps_fusion_config_and_progressive_url() {
    let extractor = ElTreceTvExtractor::new(ExtractorDescriptor::new(
        "ElTreceTVIE",
        "ElTreceTV",
        r"https?://(?:www\.)?eltrecetv\.com\.ar/[\w-]+/capitulos/temporada-\d+/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "eltrecetv.com.ar/ahora-caigo/capitulos/temporada-2023/native-chapter".to_owned(),
            br#"<html><script>
                Fusion.globalContent = {
                    "promo_items": {
                        "basic": {
                            "embed": {
                                "config": {
                                    "m3u8": "https://cdn.example/eltrece/native123.m3u8/tracks-v1a1/index.m3u8",
                                    "title": "Native El Trece Chapter",
                                    "thumbnail": "//cdn.example/eltrece/poster.jpg"
                                }
                            }
                        }
                    }
                };
            </script></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.eltrecetv.com.ar/ahora-caigo/capitulos/temporada-2023/native-chapter/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native123"));
    assert_eq!(result.get_str("title"), Some("Native El Trece Chapter"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/eltrece/poster.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(
        formats[0].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        formats[1].get("url"),
        Some(&serde_json::json!("https://cdn.example/eltrece/native123.m3u8"))
    );
}
