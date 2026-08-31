#[test]
fn newgrounds_playlist_native_extractor_materializes_collection_entries() {
    let extractor = NewgroundsPlaylistExtractor::new(ExtractorDescriptor::new(
        "NewgroundsPlaylistIE",
        "Newgrounds:playlist",
        r"https?://(?:www\.)?newgrounds\.com/(?:collection|[^/]+/search/[^/]+)/(?P<id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
                (
                    "collection/cats".to_owned(),
                    br#"<html><head><title>Cats - Newgrounds</title></head><body>
                        <a href="/portal/view/1">one</a>
                        <a href="https://www.newgrounds.com/audio/listen/2">two</a>
                    </body></html>"#
                        .to_vec(),
                ),
                (
                    "portal/view/1".to_owned(),
                    br#"<title>First - Newgrounds</title>
                        <script>embedController([{"url":"https://cdn.example/one.mp4"}]);</script>"#
                        .to_vec(),
                ),
                (
                    "audio/listen/2".to_owned(),
                    br#"<title>Second - Newgrounds</title>
                        <script>embedController([{"url":"https://cdn.example/two.mp3"}]);</script>"#
                        .to_vec(),
                ),
            ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.newgrounds.com/collection/cats", &context)
        .unwrap();
    let info = result.into_info_dict();
    assert_eq!(info.get_str("id"), Some("cats"));
    assert_eq!(info.get_str("title"), Some("Cats"));
    assert_eq!(
        info.get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn newgrounds_user_native_extractor_paginates_json_listing() {
    let extractor = NewgroundsUserExtractor::new(ExtractorDescriptor::new(
        "NewgroundsUserIE",
        "Newgrounds:user",
        r"https?://(?P<id>[^\.]+)\.newgrounds\.com/(?:movies|audio)/?(?:[#?]|$)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "burn7.newgrounds.com/audio?page=1".to_owned(),
                    br#"{"items":[["<a href=\"/audio/listen/3\">three</a>"]]}"#.to_vec(),
                ),
                (
                    "burn7.newgrounds.com/audio?page=2".to_owned(),
                    br#"{"items":[]}"#.to_vec(),
                ),
                (
                    "audio/listen/3".to_owned(),
                    br#"<title>Third - Newgrounds</title>
                        <script>embedController([{"url":"https://cdn.example/three.mp3"}]);</script>"#
                        .to_vec(),
                ),
            ],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://burn7.newgrounds.com/audio", &context)
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("burn7"));
    assert_eq!(
        result
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn wistia_native_extractor_maps_assets_thumbnails_and_subtitles() {
    let extractor = WistiaExtractor::new(ExtractorDescriptor::new(
            "WistiaIE",
            "Wistia",
            r"(?:wistia:|https?://(?:\w+\.)?wistia\.(?:net|com)/(?:embed/)?(?:iframe|medias)/)(?P<id>[a-z0-9]{10})",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"{
                "media":{
                    "hashedId":"1234567890",
                    "name":"Native Wistia",
                    "seoDescription":"Wistia description",
                    "duration":12.5,
                    "createdAt":1700000000,
                    "captions":[{"language":"en"}],
                    "assets":[
                        {"type":"original","url":"https://cdn.example/original.mp4","ext":"mp4","container":"mp4","bitrate":1200,"size":100,"width":1280,"height":720,"codec":"h264"},
                        {"type":"hls_video","display_name":"Audio","url":"https://cdn.example/stream.bin","container":"m3u8","codec":"h264"},
                        {"type":"still","url":"https://cdn.example/poster.bin","ext":"jpg","size":20,"width":1280,"height":720},
                        {"type":"preview","url":"https://cdn.example/preview.mp4","status":2}
                    ]
                }
            }"#
            .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("wistia:1234567890", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1234567890"));
    assert_eq!(result.get_str("title"), Some("Native Wistia"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(12.5)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        result
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert!(result.get("subtitles").is_some());
}

#[test]
fn wistia_playlist_native_extractor_materializes_embedded_media() {
    let extractor = WistiaPlaylistExtractor::new(ExtractorDescriptor::new(
        "WistiaPlaylistIE",
        "WistiaPlaylist",
        r"https?://(?:\w+\.)?wistia\.(?:net|com)/(?:embed/)?playlists/(?P<id>[a-z0-9]{10})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"[{"medias":[
                {"embed_config":{"media":{"hashedId":"1234567890","name":"First","assets":[{"type":"original","url":"https://cdn.example/first.mp4","ext":"mp4"}]}}},
                {"embed_config":{"media":{"hashedId":"abcdefghij","name":"Second","assets":[{"type":"original","url":"https://cdn.example/second.mp4","ext":"mp4"}]}}}
            ]}]"#
            .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://fast.wistia.net/embed/playlists/1234567890",
            &context,
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("1234567890"));
    assert_eq!(
        result
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn wistia_channel_native_extractor_materializes_video_and_episode_sections() {
    let extractor = WistiaChannelExtractor::new(ExtractorDescriptor::new(
            "WistiaChannelIE",
            "WistiaChannel",
            r"(?:wistiachannel:|https?://(?:\w+\.)?wistia\.(?:net|com)/(?:embed/)?channel/)(?P<id>[a-z0-9]{10})",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "embed/channel/1234567890.json".to_owned(),
                    br#"{"series":[{"title":"Native channel","description":"Channel description","sections":[
                        {"videos":[{"hashedId":"abcdefghij"}]},
                        {"episodes":[{"hashedId":"1234567890"}]}
                    ]}]}"#
                    .to_vec(),
                ),
                (
                    "embed/medias/abcdefghij.json".to_owned(),
                    br#"{"media":{"hashedId":"abcdefghij","name":"First","assets":[{"type":"original","url":"https://cdn.example/first.mp4","ext":"mp4"}]}}"#
                        .to_vec(),
                ),
                (
                    "embed/medias/1234567890.json".to_owned(),
                    br#"{"media":{"hashedId":"1234567890","name":"Second","assets":[{"type":"original","url":"https://cdn.example/second.mp4","ext":"mp4"}]}}"#
                        .to_vec(),
                ),
            ],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://fast.wistia.net/embed/channel/1234567890", &context)
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("1234567890"));
    assert_eq!(result.get_str("title"), Some("Native channel"));
    assert_eq!(
        result
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn vidlii_native_extractor_validates_embedded_sources_and_maps_page_fields() {
    let extractor = VidLiiExtractor::new(ExtractorDescriptor::new(
        "VidLiiIE",
        "VidLii",
        r"https?://(?:www\.)?vidlii\.com/(?:watch|embed)\?.*?\bv=(?P<id>[0-9A-Za-z_-]{11})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
                <meta name="description" content="Native description">
                <meta name="twitter:image" content="https://cdn.example/thumb.jpg">
                <meta name="datePublished" content="2021-06-12">
                <meta name="video:duration" content="89">
            </head><body>
                <h1>Native VidLii</h1>
                <script>
                    player = {src: "https://cdn.example/720.mp4"};
                    player = {src: "//cdn.example/360.mp4"};
                    img: "https://cdn.example/fallback.jpg";
                    rating: 4.5
                </script>
                <div class="wt_person"><a href="/user/NativeUser">Native User</a></div>
                <div>Category:</div><div><a href="/category/news">News &amp; Politics</a></div>
                <a href="/results?q=rust">rust</a>
            </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.vidlii.com/watch?v=tJluaH4BJ3v", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("tJluaH4BJ3v"));
    assert_eq!(result.get_str("title"), Some("Native VidLii"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(result.get_str("uploader"), Some("Native User"));
    assert_eq!(
        result.get_str("uploader_url"),
        Some("https://www.vidlii.com/user/NativeUser")
    );
    assert_eq!(result.get_str("uploader_id"), Some("NativeUser"));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1_623_456_000i64))
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(89.0)));
    assert_eq!(result.get("average_rating"), Some(&serde_json::json!(4.5)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn peertube_native_extractor_maps_api_formats_and_metadata() {
    let extractor = PeerTubeExtractor::new(ExtractorDescriptor::new(
        "PeerTubeIE",
        "PeerTube",
        r"https?://(?P<host>[^/]+)/w/(?P<id>[\da-zA-Z]{22})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"{
                "name":"Native PeerTube",
                "description":"Description",
                "publishedAt":"2024-01-02T03:04:05Z",
                "thumbnailPath":"/lazy-static/thumbnails/native.jpg",
                "duration":52,
                "views":10,
                "likes":4,
                "dislikes":1,
                "nsfw":false,
                "tags":[{"name":"ignored"},"rust"],
                "category":{"label":"Science"},
                "language":{"id":"en"},
                "licence":{"label":"CC BY"},
                "account":{"id":3,"displayName":"Native user","url":"https://peertube.test/accounts/user"},
                "channel":{"id":4,"displayName":"Native channel","url":"https://peertube.test/video-channels/channel"},
                "streamingPlaylists":[{"playlistUrl":"https://cdn.example/master.m3u8"}],
                "files":[
                    {"fileUrl":"https://cdn.example/720.mp4","size":100,"fps":30,"resolution":{"label":"720p"}},
                    {"fileUrl":"https://cdn.example/audio.mp3","resolution":{"label":"0p"}}
                ]
            }"#
            .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://peertube.test/w/3fbif9S3WmtTP8gGsC5HBd", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("3fbif9S3WmtTP8gGsC5HBd"));
    assert_eq!(result.get_str("title"), Some("Native PeerTube"));
    assert_eq!(result.get_str("uploader"), Some("Native user"));
    assert_eq!(result.get_str("channel"), Some("Native channel"));
    assert_eq!(result.get_str("language"), Some("en"));
    assert_eq!(result.get_str("license"), Some("CC BY"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(52)));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(0)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(2))
            .and_then(|format| format.get("vcodec")),
        Some(&serde_json::json!("none"))
    );
}

#[test]
fn peertube_playlist_native_extractor_paginates_and_expands_entries() {
    let extractor = PeerTubePlaylistExtractor::new(ExtractorDescriptor::new(
        "PeerTubePlaylistIE",
        "PeerTube:Playlist",
        r"https?://(?P<host>[^/]+)/(?P<type>(?:a|c|w/p))/(?P<id>[^/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "api/v1/video-playlists/list/videos?sort=-createdAt".to_owned(),
                    br#"{"data":[{"shortUUID":"one"},{"shortUUID":"two"}]}"#.to_vec(),
                ),
                (
                    "api/v1/videos/one".to_owned(),
                    br#"{"name":"Entry one","files":[{"fileUrl":"https://cdn.example/one.mp4","resolution":{"label":"720p"}}]}"#.to_vec(),
                ),
                (
                    "api/v1/videos/two".to_owned(),
                    br#"{"name":"Entry two","files":[{"fileUrl":"https://cdn.example/two.mp4","resolution":{"label":"360p"}}]}"#.to_vec(),
                ),
                (
                    "api/v1/video-playlists/list".to_owned(),
                    br#"{
                        "displayName":"Native list",
                        "description":"List description",
                        "createdAt":"2024-01-02T03:04:05Z",
                        "thumbnailPath":"/thumb.jpg",
                        "ownerAccount":{"id":9,"name":"Native owner"}
                    }"#
                    .to_vec(),
                ),
            ],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://peertube.test/w/p/list", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("playlist"));
    assert_eq!(result.get_str("id"), Some("list"));
    assert_eq!(result.get_str("title"), Some("Native list"));
    assert_eq!(result.get_str("channel"), Some("Native owner"));
    assert_eq!(result.get_str("channel_id"), Some("9"));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1_704_164_645i64))
    );
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get("title"),
        Some(&serde_json::json!("Entry one"))
    );
    assert_eq!(entries[1].get("id"), Some(&serde_json::json!("two")));
}

#[test]
fn rumble_native_extractor_merges_page_metadata_into_embed_result() {
    let extractor = RumbleExtractor::new(ExtractorDescriptor::new(
        "RumbleIE",
        "Rumble",
        r"https?://rumble\.com/(?P<id>v[a-z0-9]+)[^/]*$",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "rumble.com/vabc".to_owned(),
                    br#"<html><body>
                        <iframe src="https://rumble.com/embed/v5pv5f"></iframe>
                        Streamed on: <time datetime="2024-01-02T03:04:05Z"></time>
                        "userInteractionCount":123
                        <span data-js="rumbles_up_votes">1.5K</span>
                        <span data-js="rumbles_down_votes">2</span>
                        <div class="media-description"><p>Native description</p></div>
                    </body></html>"#
                        .to_vec(),
                ),
                (
                    "embedJS/u3".to_owned(),
                    br#"{
                        "live":0,
                        "duration":30,
                        "title":"Native Rumble",
                        "ua":{"mp4":{"720":{"url":"https://cdn.example/rumble.mp4","meta":{"h":720}}}}
                    }"#
                    .to_vec(),
                ),
            ],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://rumble.com/vabc-title.html", &context)
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("v5pv5f"));
    assert_eq!(result.get_str("title"), Some("Native Rumble"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(123)));
    assert_eq!(result.get("like_count"), Some(&serde_json::json!(1500)));
    assert_eq!(result.get("dislike_count"), Some(&serde_json::json!(2)));
    assert_eq!(
        result.get("release_timestamp"),
        Some(&serde_json::json!(1_704_164_645i64))
    );
}

#[test]
fn rumble_channel_native_extractor_paginates_and_expands_entries() {
    let extractor = RumbleChannelExtractor::new(ExtractorDescriptor::new(
        "RumbleChannelIE",
        "RumbleChannel",
        r"(?P<url>https?://(?:www\.)?rumble\.com/(?:c|user)/(?P<id>[^&?#$/]+))",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "rumble.com/c/native?page=1".to_owned(),
                    br#"<a class="videostream__link link" href="/vabc-title.html">Native video</a>"#.to_vec(),
                ),
                (
                    "rumble.com/c/native?page=2".to_owned(),
                    b"<html></html>".to_vec(),
                ),
                (
                    "rumble.com/vabc-title.html".to_owned(),
                    br#"<iframe src="https://rumble.com/embed/v5pv5f"></iframe>
                        <div class="media-description"><p>Channel entry</p></div>"#
                        .to_vec(),
                ),
                (
                    "embedJS/u3".to_owned(),
                    br#"{
                        "title":"Native Rumble",
                        "duration":30,
                        "live":0,
                        "ua":{"mp4":{"720":{"url":"https://cdn.example/rumble.mp4","meta":{"h":720}}}}
                    }"#
                    .to_vec(),
                ),
            ],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://rumble.com/c/native", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("playlist"));
    assert_eq!(result.get_str("id"), Some("native"));
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].get("id"), Some(&serde_json::json!("v5pv5f")));
    assert_eq!(
        entries[0].get("description"),
        Some(&serde_json::json!("Channel entry"))
    );
}
