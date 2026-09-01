#[test]
fn ertflix_web_native_extractor_resolves_episode_to_codename() {
    let extractor = ErtflixExtractor::new(ExtractorDescriptor::new(
        "ERTFlixIE",
        "ertflix",
        r"https?://www\.ertflix\.gr/(?:[^/]+/)?(?:series|vod)/(?P<id>[a-z]{3}\.\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "Tile/GetTiles".to_owned(),
            br#"{"Result":{"Success":true},"Tiles":[
                {"Id":"vod.173258","Codename":"native-code","Title":"Native episode",
                 "Subtitle":"Native subtitle","ShortDescription":"<p>Native description</p>",
                 "PublishDate":"2021-12-16T10:00:00Z","DurationSeconds":3166,
                 "AgeRating":"8","Images":[{"IsMain":true,"Url":"https://cdn.example/native.jpg"}]}
            ]}"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Single(info) = extractor
        .extract_with_context(
            "https://www.ertflix.gr/vod/vod.173258-native-episode",
            &context,
        )
        .unwrap()
    else {
        panic!("ERTFLIX episode should return a URL result");
    };

    assert_eq!(info.get_str("_type"), Some("url_transparent"));
    assert_eq!(info.get_str("url"), Some("ertflix:native-code"));
    assert_eq!(info.get_str("id"), Some("native-code"));
    assert_eq!(info.get_str("title"), Some("Native episode"));
    assert_eq!(info.get_str("description"), Some("Native description"));
    assert_eq!(info.get_i64("age_limit"), Some(8));
    assert_eq!(info.get_f64("duration"), Some(3166.0));
    assert_eq!(
        info.get_str("thumbnail"),
        Some("https://cdn.example/native.jpg")
    );
}

#[test]
fn ertflix_web_native_extractor_filters_and_sorts_series_episodes() {
    let extractor = ErtflixExtractor::new(ExtractorDescriptor::new(
        "ERTFlixIE",
        "ertflix",
        r"https?://www\.ertflix\.gr/(?:[^/]+/)?(?:series|vod)/(?P<id>[a-z]{3}\.\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "Tile/GetSeriesDetails".to_owned(),
            br#"{"Result":{"Success":true},
                "Series":{"Title":"Native series","ShortDescription":"Native series description",
                    "AgeRating":"8","Seasons":[{"SeasonNumber":1,"Title":"Season 1"},{"SeasonNumber":2,"Title":"Season 2"}]},
                "EpisodeGroups":[
                    {"Title":"Season 1","SeasonNumber":1,"Episodes":[{"Codename":"ignored","Title":"Ignored"}]},
                    {"Title":"Season 2","SeasonNumber":2,"Episodes":[
                        {"Codename":"episode-two","Title":"Episode two","EpisodeNumber":2},
                        {"Codename":"episode-one","Title":"Episode one","EpisodeNumber":1,"HasPlayableStream":true}
                    ]}
                ]}"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.ertflix.gr/series/ser.3448-native?season=2",
            &context,
        )
        .unwrap()
    else {
        panic!("ERTFLIX series should return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("ser.3448"));
    assert_eq!(info.get_str("title"), Some("Native series"));
    assert_eq!(info.get_i64("age_limit"), Some(8));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("url"), Some("ertflix:episode-one"));
    assert_eq!(entries[0].get_i64("episode_number"), Some(1));
    assert_eq!(entries[0].get_i64("season_number"), Some(2));
    assert_eq!(entries[1].get_str("url"), Some("ertflix:episode-two"));
}
