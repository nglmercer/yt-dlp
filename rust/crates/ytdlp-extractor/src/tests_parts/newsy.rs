#[test]
fn newsy_native_extractor_maps_player_data_hls_and_jsonld() {
    let extractor = NewsyExtractor::new(ExtractorDescriptor::new(
        "NewsyIE",
        "Newsy",
        r#"https?://(?:www\.)?newsy\.com/stories/(?P<id>[^/?#$&]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.newsy.com/stories/nft-trend-leads-to-fraudulent-art-auctions".to_owned(),
            br#"<html><head>
                    <script type="application/ld+json">{
                        "@context": "https://schema.org",
                        "@type": "VideoObject",
                        "description": "Native Newsy description",
                        "duration": "PT5M39.63S",
                        "uploadDate": "2021-05-18T12:00:00Z",
                        "thumbnailUrl": "https://cdn.newsy.com/images/jsonld.jpg"
                    }</script>
                </head><body>
                    <div data-video-player="{&quot;id&quot;:&quot;609d65125b086c24fb529312&quot;,&quot;stream&quot;:&quot;https://cdn.newsy.com/videos/nft.m3u8&quot;,&quot;headline&quot;:&quot;NFT Art Auctions Have A Piracy Problem&quot;,&quot;duration&quot;:339630,&quot;image&quot;:&quot;https://cdn.newsy.com/images/videos/x/1620927824_xyrrP4.jpg&quot;}"></div>
                </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.newsy.com/stories/nft-trend-leads-to-fraudulent-art-auctions/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("609d65125b086c24fb529312"));
    assert_eq!(
        result.get_str("display_id"),
        Some("nft-trend-leads-to-fraudulent-art-auctions")
    );
    assert_eq!(
        result.get_str("title"),
        Some("NFT Art Auctions Have A Piracy Problem")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native Newsy description")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(339.63)));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1621339200i64))
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.newsy.com/images/videos/x/1620927824_xyrrP4.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.newsy.com/videos/nft.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("m3u8_native")
    );
}
