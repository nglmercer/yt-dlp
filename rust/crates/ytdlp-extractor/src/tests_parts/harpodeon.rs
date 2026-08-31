#[test]
fn harpodeon_native_extractor_maps_page_metadata_and_mp4() {
    let extractor = HarpodeonExtractor::new(ExtractorDescriptor::new(
        "HarpodeonIE",
        "Harpodeon",
        r"https?://(?:www\.)?harpodeon\.com/(?:video|preview)/\w+/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    assert!(extractor.suitable(
        "https://www.harpodeon.com/preview/The_Smoking_Out_of_Bella_Butts/268068288"
    ));

    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "harpodeon.com/video/The_Smoking_Out_of_Bella_Butts/268068288".to_owned(),
            br#"<html><head><meta name="description" content="A &amp; native description"></head>
                <body><div class="videoInfo">
                <h2>The Smoking Out of Bella Butts</h2>
                <p>(Vitagraph Company of America, 1915)</p>
                </div><script>
                hpBase("https://cdn.example/harpodeon/");
                hpInjectVideo("268068288", "720");
                </script></body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.harpodeon.com/video/The_Smoking_Out_of_Bella_Butts/268068288",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("268068288"));
    assert_eq!(
        result.get_str("title"),
        Some("The Smoking Out of Bella Butts")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/harpodeon/268068288_720.mp4")
    );
    assert_eq!(result.get_str("description"), Some("A & native description"));
    assert_eq!(
        result.get_str("creator"),
        Some("Vitagraph Company of America")
    );
    assert_eq!(result.get_i64("release_year"), Some(1915));
    assert_eq!(
        result
            .get("http_headers")
            .and_then(|headers| headers.get("Referer")),
        Some(&serde_json::json!(
            "https://www.harpodeon.com/video/The_Smoking_Out_of_Bella_Butts/268068288"
        ))
    );
}
