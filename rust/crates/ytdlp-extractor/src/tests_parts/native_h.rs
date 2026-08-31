#[test]
fn breitbart_native_extractor_reads_page_metadata_and_manifest() {
    let extractor = BreitbartExtractor::new(ExtractorDescriptor::new(
        "BreitBartIE",
        "BreitBart",
        r"https?://(?:www\.)?breitbart\.com/videos/v/(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
                <meta property="og:title" content="Example title">
                <meta property="og:description" content="Example description">
                <meta property="og:image" content="https://cdn.example/thumb.jpg">
            </head></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.breitbart.com/videos/v/abc123/", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("title"), Some("Example title"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.jwplayer.com/manifests/abc123.m3u8")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}

#[test]
fn generic_native_extractor_reads_open_graph_and_html5_media() {
    let extractor =
        GenericExtractor::new(ExtractorDescriptor::new("GenericIE", "generic", ".*", true));
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
                <meta property="og:title" content="Native page">
                <meta property="og:description" content="Native description">
                <meta property="og:image" content="/thumb.jpg">
            </head><body>
                <video><source src="/media/video.mp4" type="video/mp4"></video>
            </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://example.test/watch/page", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("title"), Some("Native page"));
    assert_eq!(
        result.get_str("url"),
        Some("https://example.test/media/video.mp4")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://example.test/thumb.jpg")
    );
}

#[test]
fn audius_native_extractor_resolves_track_and_builds_stream_url() {
    let extractor = AudiusExtractor::new(ExtractorDescriptor::new(
            "AudiusIE",
            "Audius",
            r"(?x)https?://(?:www\.)?(?:audius\.co/(?P<uploader>[\w\d-]+)(?!/album|/playlist)/(?P<title>\S+))",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api.audius.co/".to_owned(),
                br#"{"data":["https://api.audius.test"]}"#.to_vec(),
            ),
            (
                "/v1/resolve?".to_owned(),
                br#"{
                        "data": {
                            "id": "track1",
                            "title": "Native track",
                            "description": "Description",
                            "duration": 30,
                            "genre": "Electronic",
                            "play_count": 4,
                            "favorite_count": 2,
                            "repost_count": 1,
                            "user": {"name": "artist"},
                            "artwork": {"150x150": "https://cdn.example/art.jpg"}
                        }
                    }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://audius.co/artist/native-track", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("track1"));
    assert_eq!(result.get_str("artist"), Some("artist"));
    assert_eq!(
        result.get_str("url"),
        Some("https://api.audius.test/v1/tracks/track1/stream")
    );
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(4)));
}

#[test]
fn blerp_native_extractor_reads_graphql_audio_result() {
    let extractor = BlerpExtractor::new(ExtractorDescriptor::new(
        "BlerpIE",
        "blerp",
        r"https?://(?:www\.)?blerp\.com/soundbites/(?P<id>[0-9a-zA-Z]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "api.blerp.com/graphql".to_owned(),
            br#"{
                    "data": {
                        "web": {
                            "biteById": {
                                "_id": "bite1",
                                "title": "Native sound",
                                "userKeywords": ["native", "rust"],
                                "ownerObject": {"_id": "user1", "username": "tester"},
                                "audio": {"mp3": {"url": "https://cdn.example/bite.mp3"}}
                            }
                        }
                    }
                }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://blerp.com/soundbites/6320fe8745636cb4dd677a5a",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("bite1"));
    assert_eq!(result.get_str("uploader"), Some("tester"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/bite.mp3"));
    assert_eq!(
        result
            .get("tags")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn acast_native_extractor_maps_episode_api_response() {
    let extractor = AcastExtractor::new(ExtractorDescriptor::new(
            "ACastIE",
            "acast",
            r#"(?x:https?://(?:(?:(?:embed|www|shows)\.)?acast\.com/|play\.acast\.com/s/)(?P<channel>[^/?#]+)/(?:episodes/)?(?P<id>[^/#?"]+))"#,
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "feeder.acast.com/api/v1/shows".to_owned(),
            br#"{
                    "id": "episode1",
                    "episodeUrl": "episode-slug",
                    "url": "https://cdn.example/episode.mp3",
                    "title": "Native episode",
                    "description": "Description",
                    "duration": 120,
                    "show": {"author": "Creator", "title": "Series"},
                    "season": 2,
                    "episode": 4
                }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://shows.acast.com/channel/episodes/episode-slug",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("episode1"));
    assert_eq!(result.get_str("series"), Some("Series"));
    assert_eq!(result.get_str("creator"), Some("Creator"));
    assert_eq!(result.get("episode_number"), Some(&serde_json::json!(4)));
}

#[test]
fn acast_channel_native_extractor_builds_playlist_entries() {
    let extractor = AcastChannelExtractor::new(ExtractorDescriptor::new(
        "ACastChannelIE",
        "acast:channel",
        r"(?x)https?://(?:(?:(?:www|shows)\.)?acast\.com/|play\.acast\.com/s/)(?P<id>[^/#?]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "feeder.acast.com/api/v1/shows".to_owned(),
            br#"{
                    "id": "show1",
                    "title": "Native show",
                    "description": "Show description",
                    "author": "Creator",
                    "episodes": [
                        {"id": "episode1", "title": "One", "url": "https://cdn.example/one.mp3"},
                        {"id": "episode2", "title": "Two", "url": "https://cdn.example/two.mp3"}
                    ]
                }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://shows.acast.com/native-show", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("playlist"));
    assert_eq!(result.get_str("title"), Some("Native show"));
    assert_eq!(
        result
            .get("entries")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn dumpert_native_extractor_maps_variants_and_stats() {
    let extractor = DumpertExtractor::new(ExtractorDescriptor::new(
            "DumpertIE",
            "Dumpert",
            r"(?x)(?P<protocol>https?)://(?:(?:www|legacy)\.)?dumpert\.nl/(?:item/)(?P<id>[0-9]+[/_][0-9a-zA-Z]+)",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "api-live.dumpert.nl/mobile_api/json/info".to_owned(),
            br#"{
                    "items": [{
                        "title": "Native Dumpert",
                        "description": "Description",
                        "media": [{
                            "mediatype": "VIDEO",
                            "duration": 9,
                            "variants": [
                                {"version": "mobile", "uri": "https://cdn.example/mobile.mp4"},
                                {"version": "hls", "uri": "https://cdn.example/master.m3u8"}
                            ]
                        }],
                        "stills": {"thumb": "https://cdn.example/thumb.jpg"},
                        "stats": {"kudos_total": 3, "views_total": 10}
                    }]
                }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.dumpert.nl/item/6646981_951bc60f", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("6646981/951bc60f"));
    assert_eq!(result.get_str("title"), Some("Native Dumpert"));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(10)));
    assert_eq!(
        result
            .get("formats")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn audiodraft_native_extractor_posts_entry_lookup() {
    let extractor = AudiodraftExtractor::new(ExtractorDescriptor::new(
        "AudiodraftGenericIE",
        "Audiodraft:generic",
        r"https?://www\.audiodraft\.com/contests/[^/#]+#entries&eid=(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "audiodraft.com/scripts/general/player/getPlayerInfoNew.php".to_owned(),
            br#"{
                    "entry_id": 30138,
                    "entry_title": "Native sound",
                    "path": "https://cdn.example/sound.mp3",
                    "designer_name": "tester",
                    "designer_id": 19452,
                    "entry_url": "https://www.audiodraft.com/entry/30138",
                    "entry_likes": 7,
                    "entry_rating": 5
                }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.audiodraft.com/contests/contest#entries&eid=30138",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("30138"));
    assert_eq!(result.get_str("uploader"), Some("tester"));
    assert_eq!(result.get("average_rating"), Some(&serde_json::json!(5)));
}

#[test]
fn audiomack_native_extractor_maps_song_api_response() {
    let extractor = AudiomackExtractor::new(ExtractorDescriptor::new(
        "AudiomackIE",
        "audiomack",
        r"https?://(?:www\.)?audiomack\.com/(?:song/|(?=.+/song/))(?P<id>[\w/-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "audiomack.com/api/music/url/song".to_owned(),
            br#"{
                    "id": 310086,
                    "artist": "Native artist",
                    "title": "Native song",
                    "url": "https://cdn.example/song.mp3"
                }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.audiomack.com/song/native-artist/native-song",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("310086"));
    assert_eq!(result.get_str("uploader"), Some("Native artist"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
}

#[test]
fn aitube_native_extractor_reads_next_data_and_hls_result() {
    let extractor = AitubeExtractor::new(ExtractorDescriptor::new(
        "AitubeKZVideoIE",
        "AitubeKZVideo",
        r"https?://aitube\.kz/(?:video|embed/)\?(?:[^\?]+)?id=(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head></head><body>
                <script id="__NEXT_DATA__" type="application/json">{
                    "props": {"pageProps": {"videoInfo": {
                        "title": "Native Aitube",
                        "description": "Description",
                        "viewCount": 12,
                        "channelTitle": "Channel",
                        "channelId": "channel1",
                        "coverUrl": "https://cdn.example/cover.jpg"
                    }}}
                }</script>
            </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://aitube.kz/video?id=9291d29b-c038-49a1-ad42-3da2051d353c",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("title"), Some("Native Aitube"));
    assert_eq!(result.get_str("channel"), Some("Channel"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert!(
        result
            .get_str("url")
            .is_some_and(|url| url.ends_with("/video"))
    );
}

#[test]
fn art19_native_extractor_maps_player_and_rss_metadata() {
    let extractor = Art19Extractor::new(ExtractorDescriptor::with_valid_urls(
        "Art19IE",
        "Art19",
        vec![
            r"https?://(?:www\.)?art19\.com/shows/[^/#?]+/episodes/(?P<id>[\da-f]{8}-?[\da-f]{4}-?[\da-f]{4}-?[\da-f]{4}-?[\da-f]{12})"
                .to_owned(),
            r"https?://rss\.art19\.com/episodes/(?P<id>[\da-f]{8}-?[\da-f]{4}-?[\da-f]{4}-?[\da-f]{4}-?[\da-f]{12})\.mp3"
                .to_owned(),
        ],
        true,
    ))
    .unwrap();
    let episode_id = "5ba1413c-48b8-472b-9cc3-cfd952340bdb";
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                format!("https://art19.com/episodes/{episode_id}"),
                br#"{
                    "episode": {
                        "title": "Native Art19 episode",
                        "description_plain": "Player description",
                        "id": "5ba1413c-48b8-472b-9cc3-cfd952340bdb",
                        "created_at": "2024-01-22T12:26:55Z",
                        "released_at": "2024-01-22T12:31:15Z",
                        "updated_at": "2024-01-22T12:34:35Z"
                    }
                }"#
                .to_vec(),
            ),
            (
                format!("https://rss.art19.com/episodes/{episode_id}.json"),
                br#"{
                    "content": {
                        "episode_title": "Native RSS title",
                        "episode_description_plain": "RSS description",
                        "episode_id": "5ba1413c-48b8-472b-9cc3-cfd952340bdb",
                        "episode_number": 582,
                        "series_title": "Native series",
                        "series_id": "series-1",
                        "season_title": "Season 2",
                        "season_id": "season-2",
                        "season_number": 2,
                        "cover_image": "https://cdn.example/cover.jpg",
                        "duration": 527.4,
                        "media": {
                            "mp3": {"url": "https://cdn.example/episode.mp3"},
                            "ogg": {"url": "https://cdn.example/episode.ogg"},
                            "waveform_bin": {"url": "https://cdn.example/waveform.bin"}
                        }
                    }
                }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert!(extractor.suitable(&format!("https://rss.art19.com/episodes/{episode_id}.mp3")));
    let result = extractor
        .extract_with_context(
            &format!("https://art19.com/shows/example/episodes/{episode_id}"),
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some(episode_id));
    assert_eq!(result.get_str("title"), Some("Native RSS title"));
    assert_eq!(result.get_str("series"), Some("Native series"));
    assert_eq!(result.get_i64("episode_number"), Some(582));
    assert_eq!(result.get_f64("duration"), Some(527.4));
    assert_eq!(
        result.get_i64("timestamp"),
        yt_dlp_core::parse_iso8601("2024-01-22T12:26:55Z")
    );
    let formats = result.get("formats").and_then(serde_json::Value::as_array);
    assert_eq!(formats.map(Vec::len), Some(3));
    assert!(formats.is_some_and(|formats| {
        formats
            .iter()
            .all(|format| format.get("vcodec") == Some(&serde_json::json!("none")))
    }));
}
