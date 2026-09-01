#[test]
fn eggs_song_native_extractor_maps_direct_audio_metadata() {
    let extractor = EggsExtractor::new(ExtractorDescriptor::new(
        "EggsIE",
        "eggs:single",
        r"https?://eggs\.mu/artist/[^/?#]+/song/(?P<id>[\da-f-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
            "musicId":"0e95fd1d-4d61-4d5b-8b18-6092c551da90",
            "musicTitle":"Native song",
            "musicDataPath":"https://cdn.example/song.m4a",
            "artist":{"artistName":"32_sunny_girl","displayName":"Sunny Girl","artistId":1607},
            "imageDataPath":"https://cdn.example/song.jpg",
            "numberOfMusicPlays":10,
            "numberOfLikes":3,
            "numberOfComments":2,
            "composer":"Native Composer",
            "tags":["Native","Rust"],
            "releaseDate":"2024-11-11T00:00:00Z"
        }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://eggs.mu/artist/32_sunny_girl/song/0e95fd1d-4d61-4d5b-8b18-6092c551da90",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("0e95fd1d-4d61-4d5b-8b18-6092c551da90")
    );
    assert_eq!(result.get_str("title"), Some("Native song"));
    assert_eq!(result.get_str("uploader"), Some("Sunny Girl"));
    assert_eq!(result.get_str("uploader_id"), Some("1607"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/song.m4a"));
    assert_eq!(result.get_str("ext"), Some("m4a"));
    assert_eq!(result.get_i64("view_count"), Some(10));
    assert_eq!(result.get_i64("like_count"), Some(3));
    assert_eq!(result.get_i64("comment_count"), Some(2));
    assert!(result.get_i64("timestamp").is_some());
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("vcodec")),
        Some(&serde_json::json!("none"))
    );
}

#[test]
fn eggs_song_native_extractor_redirects_youtube_records() {
    let extractor = EggsExtractor::new(ExtractorDescriptor::new(
        "EggsIE",
        "eggs:single",
        r"https?://eggs\.mu/artist/[^/?#]+/song/(?P<id>[\da-f-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"youtubeUrl":"https://www.youtube.com/watch?v=Native12345"}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        extractor
            .extract_with_context(
                "https://eggs.mu/artist/band/song/0e95fd1d-4d61-4d5b-8b18-6092c551da90",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "https://www.youtube.com/watch?v=Native12345".to_owned(),
            ie_key: Some("Youtube".to_owned()),
        }
    );
}

#[test]
fn eggs_artist_native_extractor_builds_direct_and_youtube_entries() {
    let extractor = EggsArtistExtractor::new(ExtractorDescriptor::new(
        "EggsArtistIE",
        "eggs:artist",
        r"https?://eggs\.mu/artist/(?P<id>\w+)/?(?:[?#&]|$)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "artists/native_band/musics".to_owned(),
                br#"{"data":[
                    {"musicId":"song-one","musicTitle":"One","musicDataPath":"https://cdn.example/one.m4a","artist":{"displayName":"Native Band"}},
                    {"youtubeUrl":"https://www.youtube.com/watch?v=Native12345"}
                ]}"#
                .to_vec(),
            ),
            (
                "artists/native_band".to_owned(),
                br#"{"displayName":"Native Band","profile":"Band profile","imageDataPath":"https://cdn.example/band.jpg"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://eggs.mu/artist/native_band", &context)
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("Eggs artist must return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("native_band"));
    assert_eq!(info.get_str("title"), Some("Native Band"));
    assert_eq!(info.get_str("description"), Some("Band profile"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("song-one"));
    assert_eq!(entries[0].get_str("url"), Some("https://cdn.example/one.m4a"));
    assert_eq!(entries[1].get_str("_type"), Some("url"));
    assert_eq!(entries[1].get_str("ie_key"), Some("Youtube"));
}
