#[test]
fn epidemic_sound_native_extractor_maps_catalog_metadata_stems_and_thumbnails() {
    let extractor = EpidemicSoundExtractor::new(ExtractorDescriptor::new(
        "EpidemicSoundIE",
        "EpidemicSound",
        r"https?://(?:www\.)?epidemicsound\.com/(?:(?P<sfx>sound-effects/tracks)|track)/(?P<id>[0-9a-zA-Z-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "json/track/native-slug".to_owned(),
            br#"{
                "id":45014,"publicSlug":"native-slug","title":"Native track","oldTitle":"Old native track",
                "length":237,"added":"2023-09-11T00:00:00Z","releaseDate":"2023-11-21T00:00:00Z",
                "genres":[{"tag":"drum and bass"}],"metadataTags":[{"tag":"energetic"},{"tag":"liquid"}],
                "isExplicit":true,"imageUrl":"https://cdn.example/cover/3000x3000.jpg",
                "coverArt":{"baseUrl":"https://cdn.example/","sizes":{"small":"100x100.jpg","large":"1000x1000.jpg"}},
                "stems":{
                    "full":{"format":"full","s3TrackId":"full-id","stemType":"full","lqMp3Url":"https://cdn.example/full.mp3"},
                    "drums":{"format":"preview","s3TrackId":"drums-id","stemType":"drums","lqMp3Url":"https://cdn.example/drums.mp3"}
                }
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.epidemicsound.com/track/native-slug/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("45014"));
    assert_eq!(result.get_str("display_id"), Some("native-slug"));
    assert_eq!(result.get_str("title"), Some("Native track"));
    assert_eq!(result.get_str("alt_title"), Some("Old native track"));
    assert_eq!(result.get_f64("duration"), Some(237.0));
    assert_eq!(result.get_i64("age_limit"), Some(18));
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["drum and bass"]))
    );
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["energetic", "liquid"]))
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
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
}

#[test]
fn epidemic_sound_native_extractor_uses_kosmos_endpoint_for_sound_effects() {
    let extractor = EpidemicSoundExtractor::new(ExtractorDescriptor::new(
        "EpidemicSoundIE",
        "EpidemicSound",
        r"https?://(?:www\.)?epidemicsound\.com/(?:(?P<sfx>sound-effects/tracks)|track)/(?P<id>[0-9a-zA-Z-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "json/track/kosmos-id/sfx-id".to_owned(),
            br#"{"id":"208931","publicSlug":"sfx-id","title":"Native effect","length":1.0,
                "stems":{"full":{"format":"full","stemType":"full","lqMp3Url":"https://cdn.example/effect.mp3"}}}"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.epidemicsound.com/sound-effects/tracks/sfx-id/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("208931"));
    assert_eq!(result.get_str("title"), Some("Native effect"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/effect.mp3"));
}
