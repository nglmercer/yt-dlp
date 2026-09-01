#[test]
fn freetv_movies_native_extractor_maps_ajax_hls() {
    let extractor = FreeTvMoviesExtractor::new(ExtractorDescriptor::new(
        "FreeTvMoviesIE",
        "FreeTvMovies",
        r"https?://(?:www\.)?freetv\.com/peliculas/(?P<id>[^/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "freetv.com/peliculas/native-movie".to_owned(),
                br#"<div class="postid-428021"></div>"#.to_vec(),
            ),
            (
                "admin-ajax.php".to_owned(),
                br#"{"data":{"displayMeta":{"contentID":"428021","streamURLVideo":"https://cdn.example/freetv/movie.m3u8","title":"Native movie","desc":"Movie description"}}}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.freetv.com/peliculas/native-movie", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("428021"));
    assert_eq!(result.get_str("title"), Some("Native movie"));
    assert_eq!(
        result.get_str("description"),
        Some("Movie description")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/freetv/movie.m3u8")
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
fn freetv_series_native_extractor_builds_episode_playlist() {
    let extractor = FreeTvExtractor::new(ExtractorDescriptor::new(
        "FreeTvIE",
        "freetv:series",
        r"https?://(?:www\.)?freetv\.com/series/(?P<id>[^/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "freetv.com/series/native-series".to_owned(),
                br#"<h1 class="synopis">Native Series</h1>
                    <div class="synopis content"><p>Series description</p></div>
                    <select><option value="10">Season 1</option></select>"#
                    .to_vec(),
            ),
            (
                "admin-ajax.php".to_owned(),
                br#"{"data":{"1":[
                    {"contentID":"1001","fullTitle":"Native episode 1","description":"Episode one","thumbnail":"https://cdn.example/one.jpg","streamURL":"https://cdn.example/one.m3u8","contentMeta":{"displayMeta":{"seriesID":"native-series","seasonID":"10","seasonNum":"1","episodeNum":"1"}}},
                    {"contentID":1002,"fullTitle":"Native episode 2","streamURL":"https://cdn.example/two.m3u8","contentMeta":{"displayMeta":{"seriesID":"native-series","seasonID":"10","seasonNum":1,"episodeNum":2}}}
                ]}}"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("https://www.freetv.com/series/native-series", &context)
        .unwrap()
    else {
        panic!("FreeTV series should return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("native-series"));
    assert_eq!(info.get_str("title"), Some("Native Series"));
    assert_eq!(info.get_str("description"), Some("Series description"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("1001"));
    assert_eq!(entries[0].get_str("series"), Some("Native Series"));
    assert_eq!(entries[0].get_i64("season_number"), Some(1));
    assert_eq!(entries[1].get_str("id"), Some("1002"));
    assert_eq!(entries[1].get_i64("episode_number"), Some(2));
}
