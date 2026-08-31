#[test]
fn netzkino_native_extractor_maps_next_state_movie_and_mpd() {
    let extractor = NetzkinoExtractor::new(ExtractorDescriptor::new(
        "NetzkinoIE",
        "Netzkino",
        r"https?://(?:www\.)?netzkino\.de/details/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let page_state = serde_json::json!({
        "props": {
            "__dehydratedState": {
                "queries": [{
                    "state": {
                        "data": {
                            "data": {
                                "__typename": "CmsMovie",
                                "originalTitle": "Snow <b>Beast</b>",
                                "fskRating": 12,
                                "longSynopsis": "<p>A native synopsis</p>",
                                "runtimeInSeconds": 3600,
                                "productionCountry": "US",
                                "productionYear": 2011,
                                "coverImage": {"masterUrl": "/covers/snow-beast.jpg"},
                                "videoSource": {"pmdUrl": "movies/snow-beast/manifest.mpd"},
                                "cast": {"nodes": [{"person": {"name": "Actor One"}}]},
                                "directors": {"nodes": [{"person": {"name": "Director One"}}]},
                                "writers": {"nodes": [{"person": {"name": "Writer One"}}]},
                                "categories": {"nodes": [{"category": {"title": "Drama"}}]}
                            }
                        }
                    }
                }]
            }
        }
    });
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.netzkino.de/details/snow-beast".to_owned(),
            format!("<script id=\"__NEXT_DATA__\" type=\"application/json\">{page_state}</script>")
                .into_bytes(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.netzkino.de/details/snow-beast", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("snow-beast"));
    assert_eq!(result.get_str("title"), Some("Snow Beast"));
    assert_eq!(result.get_str("description"), Some("A native synopsis"));
    assert_eq!(result.get_i64("age_limit"), Some(12));
    assert_eq!(result.get_i64("duration"), Some(3600));
    assert_eq!(result.get_i64("release_year"), Some(2011));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://www.netzkino.de/covers/snow-beast.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://pmd.netzkino-seite.netzkino.de/movies/snow-beast/manifest.mpd")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("http_dash_segments")
    );
    assert_eq!(
        result.get("cast"),
        Some(&serde_json::json!(["Actor One"]))
    );
    assert_eq!(
        result.get("creators"),
        Some(&serde_json::json!(["Director One", "Writer One"]))
    );
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["Drama"]))
    );
}
