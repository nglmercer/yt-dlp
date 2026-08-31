#[test]
fn slideshare_native_extractor_reads_embedded_video_json() {
    let extractor = SlideshareExtractor::new(ExtractorDescriptor::new(
        "SlideshareIE",
        "Slideshare",
        r"https?://(?:www\.)?slideshare\.net/[^/]+?/(?P<title>.+?)($|\?)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><body>
                <script>$.extend(window.slideshare_object, slideshare_object, {
                    "slideshow":{"type":"video","id":25665706,
                        "title":"Managing Scale and Complexity",
                        "pin_image_url":"https://cdn.example/thumb.jpg"},
                    "doc":"managing-scale",
                    "jsplayer":{"video_bucket":"https://cdn.example/videos/",
                        "video_extension":"mp4"}
                });</script>
                <div id="slideshow-description-paragraph">
                    <p>Presentation description</p>
                </div>
            </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.slideshare.net/Dataversity/keynote-presentation",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("25665706"));
    assert_eq!(
        result.get_str("title"),
        Some("Managing Scale and Complexity")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/videos/managing-scale-SD.mp4")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Presentation description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/thumb.jpg")
    );
}

#[test]
fn soundgasm_native_extractor_maps_audio_metadata() {
    let extractor = SoundgasmExtractor::new(ExtractorDescriptor::new(
            "SoundgasmIE",
            "soundgasm",
            r"https?://(?:www\.)?soundgasm\.net/u/(?P<user>[0-9a-zA-Z_-]+)/(?P<display_id>[0-9a-zA-Z_-]+)",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div class="jp-title">Piano sample</div>
                <div class="jp-description">Royal Free Sample Music</div>
                <script>const media = {m4a: "https://cdn.example/88abd86.m4a"};</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://soundgasm.net/u/ytdl/Piano-sample", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("88abd86"));
    assert_eq!(result.get_str("display_id"), Some("Piano-sample"));
    assert_eq!(result.get_str("title"), Some("Piano sample"));
    assert_eq!(
        result.get_str("description"),
        Some("Royal Free Sample Music")
    );
    assert_eq!(result.get_str("uploader"), Some("ytdl"));
    assert_eq!(result.get_str("ext"), Some("m4a"));
    assert_eq!(result.get_str("vcodec"), Some("none"));
}

#[test]
fn soundgasm_profile_native_extractor_materializes_audio_entries() {
    let extractor = SoundgasmProfileExtractor::new(ExtractorDescriptor::new(
        "SoundgasmProfileIE",
        "soundgasm:profile",
        r"https?://(?:www\.)?soundgasm\.net/u/(?P<id>[^/]+)/?(?:\#.*)?$",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "soundgasm.net/u/ytdl/Piano-sample".to_owned(),
                br#"<div class="jp-title">Piano sample</div>
                        <script>const media = {m4a: "https://cdn.example/piano.m4a"};</script>"#
                    .to_vec(),
            ),
            (
                "soundgasm.net/u/ytdl".to_owned(),
                br#"<a href="/u/ytdl/Piano-sample">Piano</a>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://soundgasm.net/u/ytdl", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("playlist"));
    assert_eq!(result.get_str("id"), Some("ytdl"));
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].get("title"),
        Some(&serde_json::json!("Piano sample"))
    );
}

#[test]
fn imgur_native_extractor_maps_animated_media_and_metadata() {
    let extractor = ImgurExtractor::new(ExtractorDescriptor::new(
            "ImgurIE",
            "imgur",
            r"https?://(?:i\.)?imgur\.com/(?!(?:a|gallery|t|topic|r)/)(?:[^/?#]+-)?(?P<id>[a-zA-Z0-9]+)",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![(
                "api.imgur.com/post/v1/media/A61SaA1".to_owned(),
                br#"{
                    "media":[{
                        "type":"video",
                        "url":"https://cdn.example/A61SaA1.mp4",
                        "ext":"mp4",
                        "width":640,
                        "height":360,
                        "size":1234,
                        "metadata":{
                            "title":"Animated post",
                            "description":"Native Imgur description",
                            "duration":12.5,
                            "created_at":"2024-01-02T03:04:05Z",
                            "has_sound":true
                        }
                    }],
                    "account_id":7,
                    "account":{"username":"native-user","avatar_url":"https://cdn.example/avatar.jpg"},
                    "upvote_count":11,
                    "downvote_count":2,
                    "comment_count":4,
                    "is_mature":false,
                    "created_at":"2024-01-02T03:04:05Z"
                }"#
                .to_vec(),
            )],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://imgur.com/A61SaA1", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("A61SaA1"));
    assert_eq!(result.get_str("title"), Some("Animated post"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Imgur description")
    );
    assert_eq!(result.get_str("uploader"), Some("native-user"));
    assert_eq!(result.get("like_count"), Some(&serde_json::json!(11)));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(12.5)));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1_704_164_645i64))
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn imgur_gallery_native_extractor_expands_animated_entries() {
    let extractor = ImgurGalleryExtractor::new(
            ExtractorDescriptor::new(
                "ImgurGalleryIE",
                "imgur:gallery",
                r"https?://(?:i\.)?imgur\.com/(?:gallery|(?:t(?:opic)?|r)/[^/?#]+)/(?:[^/?#]+-)?(?P<id>[a-zA-Z0-9]+)",
                true,
            ),
            true,
        )
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "api.imgur.com/post/v1/albums/gallery".to_owned(),
                    br#"{
                        "is_album":true,
                        "title":"Native gallery",
                        "description":"Gallery description",
                        "media":[
                            {"id":"one","type":"video"},
                            {"id":"two","metadata":{"is_animated":true}}
                        ]
                    }"#
                    .to_vec(),
                ),
                (
                    "api.imgur.com/post/v1/media/one".to_owned(),
                    br#"{"media":[{"type":"video","url":"https://cdn.example/one.mp4","metadata":{"title":"One"}}]}"#
                        .to_vec(),
                ),
                (
                    "api.imgur.com/post/v1/media/two".to_owned(),
                    br#"{"media":[{"type":"video","url":"https://cdn.example/two.mp4","metadata":{"title":"Two"}}]}"#
                        .to_vec(),
                ),
            ],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://imgur.com/gallery/gallery", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("playlist"));
    assert_eq!(result.get_str("title"), Some("Native gallery"));
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get("id"), Some(&serde_json::json!("one")));
    assert_eq!(entries[1].get("title"), Some(&serde_json::json!("Two")));
}

#[test]
fn ebaumsworld_native_extractor_maps_xml_player_fields() {
    let extractor = EbaumsWorldExtractor::new(ExtractorDescriptor::new(
        "EbaumsWorldIE",
        "EbaumsWorld",
        r"https?://(?:www\.)?ebaumsworld\.com/videos/[^/]+/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<video>
                <file>https://cdn.example/83367677.mp4</file>
                <title>A Giant Python Opens The Door</title>
                <description>This is how nightmares start...</description>
                <image>https://cdn.example/thumb.jpg</image>
                <username>jihadpizza</username>
            </video>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.ebaumsworld.com/videos/a-giant-python-opens-the-door/83367677/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("83367677"));
    assert_eq!(
        result.get_str("title"),
        Some("A Giant Python Opens The Door")
    );
    assert_eq!(
        result.get_str("description"),
        Some("This is how nightmares start...")
    );
    assert_eq!(result.get_str("uploader"), Some("jihadpizza"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/thumb.jpg")
    );
}

#[test]
fn fuyin_native_extractor_maps_api_media_and_page_description() {
    let extractor = FuyinTvExtractor::new(ExtractorDescriptor::new(
        "FuyinTVIE",
        "FuyinTV",
        r"https?://(?:www\.)?fuyin\.tv/html/(?:\d+)/(?P<id>\d+)\.html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "api/api/tv.movie/url?urlid=44129".to_owned(),
                r#"{"data":{"title":"第1集","url":"https://cdn.example/episode.mp4"}}"#
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "www.fuyin.tv/html/2733/44129.html".to_owned(),
                br#"<meta name="description" content="Native description">"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.fuyin.tv/html/2733/44129.html", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("44129"));
    assert_eq!(result.get_str("title"), Some("第1集"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/episode.mp4")
    );
    assert_eq!(result.get_str("description"), Some("Native description"));
}

#[test]
fn cam4_native_extractor_maps_live_hls_stream() {
    let extractor = Cam4Extractor::new(ExtractorDescriptor::new(
        "CAM4IE",
        "CAM4",
        r"https?://(?:[^/]+\.)?cam4\.com/(?P<id>[a-z0-9_]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"cdnURL":"https://cdn.example/foxynesss.m3u8"}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.cam4.com/foxynesss", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("foxynesss"));
    assert_eq!(result.get_str("title"), Some("foxynesss"));
    assert_eq!(result.get_str("live_status"), Some("is_live"));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(18)));
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
fn kommunetv_native_extractor_maps_stream_api_and_removes_query() {
    let extractor = KommunetvExtractor::new(ExtractorDescriptor::new(
        "KommunetvIE",
        "Kommunetv",
        r"https?://\w+\.kommunetv\.no/archive/(?P<id>\w+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: r#"{
                "stream":{"title":"Bystyremøte"},
                "playlist":[{"playlist":[{"file":"https://cdn.example/meeting.m3u8?token=1"}]}]
            }"#
        .as_bytes()
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://oslo.kommunetv.no/archive/921", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("921"));
    assert_eq!(result.get_str("title"), Some("Bystyremøte"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/meeting.m3u8")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}

#[test]
fn stream_cz_native_extractor_maps_graphql_playlist_and_subtitles() {
    let extractor = StreamCzExtractor::new(ExtractorDescriptor::new(
            "StreamCZIE",
            "StreamCZ",
            r"https?://(?:www\.)?(?:stream|televizeseznam)\.cz/[^?#]+/(?P<display_id>[^?#]+)-(?P<id>[0-9]+)",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "televizeseznam.cz/api/graphql".to_owned(),
                br#"{
                        "data":{"episode":{
                            "id":"57953890",
                            "spl":"https://cdn.example/playlist/",
                            "name":"Native Stream",
                            "perex":"Native description",
                            "duration":50.2,
                            "views":7
                        }}
                    }"#
                .to_vec(),
            ),
            (
                "cdn.example/playlist/spl2,3".to_owned(),
                br#"{
                        "data":{
                            "http_stream":{"qualities":{
                                "480p":{"url":"media/480.ts","bandwidth":500000,
                                    "duration":50200,"resolution":[854,480],"codec":"h264+aac"}
                            }},
                            "mp4":{"720p":{"url":"media/720.mp4","bandwidth":1000000,
                                "duration":50200,"resolution":[1280,720],"codec":"h264+aac"}},
                            "subtitles":{"en":{"language":"en",
                                "urls":{"vtt":"subtitles/en.vtt"}}}
                        }
                    }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.televizeseznam.cz/video/lajna/native-stream-57953890",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("57953890"));
    assert_eq!(result.get_str("display_id"), Some("native-stream"));
    assert_eq!(result.get_str("title"), Some("Native Stream"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(50.2)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!(
            "https://cdn.example/playlist/subtitles/en.vtt"
        ))
    );
}

#[test]
fn vidyard_native_extractor_maps_player_sources_captions_and_metadata() {
    let extractor = VidyardExtractor::new(ExtractorDescriptor::with_valid_urls(
        "VidyardIE",
        "Vidyard",
        vec![
            r"https?://[\w-]+(?:\.hubs)?\.vidyard\.com/watch/(?P<id>[\w-]+)".to_owned(),
            r"https?://(?:embed|share)\.vidyard\.com/share/(?P<id>[\w-]+)".to_owned(),
            r"https?://play\.vidyard\.com/(?:player/)?(?P<id>[\w-]+)".to_owned(),
        ],
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"{
                "payload":{
                    "playerUuid":"player1",
                    "chapters":[{
                        "facadeUuid":"facade1",
                        "videoId":50347,
                        "name":"Native Vidyard",
                        "description":"Description &amp; details",
                        "milliseconds":99000,
                        "sources":{
                            "hls":[{"profile":"auto","url":"https://cdn.example/master.m3u8"}],
                            "mp4":[{"profile":"720p","url":"https://cdn.example/video.mp4","mimeType":"video/mp4"}]
                        },
                        "captions":[{"language":"en","name":"English","vttUrl":"https://cdn.example/en.vtt"}],
                        "thumbnailUrls":{"small":{"url":"https://cdn.example/small.jpg"}},
                        "tags":[{"name":"native"}]
                    }]
                }
            }"#
            .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://play.vidyard.com/player/oTDMPlUv--51Th455G5u7Q.json",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("facade1"));
    assert_eq!(result.get_str("display_id"), Some("50347"));
    assert_eq!(result.get_str("title"), Some("Native Vidyard"));
    assert_eq!(result.get_str("description"), Some("Description & details"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(99.0)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(result.get("subtitles").is_some());
    assert_eq!(
        result
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .and_then(|tags| tags.first())
            .and_then(serde_json::Value::as_str),
        Some("native")
    );
}

#[test]
fn audioboom_native_extractor_reads_embedded_clip_store() {
    let extractor = AudioBoomExtractor::new(ExtractorDescriptor::new(
        "AudioBoomIE",
        "AudioBoom",
        r"https?://(?:www\.)?audioboom\.com/(?:boos|posts)/(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<html><head>
                <meta property="og:description" content="fallback description">
                <meta property="weibo:audio:duration" content="12.5">
            </head><body>
                <div data-react-class="V5DetailPagePlayer"
                  data-react-props="{&quot;clips&quot;:[{&quot;clipURLPriorToLoading&quot;:&quot;https://cdn.example/audio.mp3&quot;,&quot;title&quot;:&quot;Native audio&quot;,&quot;description&quot;:&quot;Clip description&quot;,&quot;duration&quot;:12.25,&quot;author&quot;:&quot;Native host&quot;}]}"></div>
            </body></html>"#
                .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://audioboom.com/posts/12345-title", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("12345"));
    assert_eq!(result.get_str("title"), Some("Native audio"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/audio.mp3"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(12.25)));
    assert_eq!(result.get_str("uploader"), Some("Native host"));
}
