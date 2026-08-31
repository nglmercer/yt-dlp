#[test]
fn apa_native_extractor_returns_jwplatform_redirect() {
    let extractor = ApaExtractor::new(ExtractorDescriptor::new(
            "APAIE",
            "APA",
            r"(?P<base_url>https?://[^/]+\.apa\.at)/embed/(?P<id>[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12})",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"mediaId: "Abc12345""#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let extraction = extractor
        .extract_with_context(
            "https://uvp.apa.at/embed/293f6d17-692a-44e3-9fd5-7b178f3a1029",
            &context,
        )
        .unwrap();

    assert_eq!(
        extraction,
        ExtractorResult::Redirect {
            url: "jwplatform:Abc12345".to_owned(),
            ie_key: Some("JWPlatform".to_owned()),
        }
    );
}

#[test]
fn movingimage_native_extractor_maps_hls_and_archive_fields() {
    let extractor = MovingImageExtractor::new(ExtractorDescriptor::new(
        "MovingImageIE",
        "MovingImage",
        r"https?://movingimage\.nls\.uk/film/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<span class="field_title">Title:</span>
                    <span class="field_content">(SHETLAND WOOL)</span>
                <span class="field_title">Description:</span>
                    <span class="field_content">Native archive description</span>
                <span class="field_title">Running time:</span>
                    <span class="field_content">00:15:00</span>
                <script>file: "https://cdn.example/3561.m3u8";
                    image: 'https://cdn.example/3561.jpg'</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://movingimage.nls.uk/film/3561", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("3561"));
    assert_eq!(result.get_str("title"), Some("SHETLAND WOOL"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(900.0)));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/3561.jpg")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}

#[test]
fn tweakers_native_extractor_maps_progressive_api_formats() {
    let extractor = TweakersExtractor::new(ExtractorDescriptor::new(
        "TweakersIE",
        "Tweakers",
        r"https?://tweakers\.net/video/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"items":[{
                "title":"Native Tweakers video",
                "description":"Native description",
                "poster":"https://cdn.example/poster.jpg",
                "duration":386,
                "account":"s7JeEm",
                "locations":{"progressive":[{
                    "label":"720p","width":1280,"height":720,
                    "sources":[{"src":"https://cdn.example/tweakers.mp4","type":"video/mp4"}]
                }]}
            }]}"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://tweakers.net/video/9926/new-nintendo-3ds-xl-op-alle-fronten-beter.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("9926"));
    assert_eq!(result.get_str("title"), Some("Native Tweakers video"));
    assert_eq!(result.get_str("uploader_id"), Some("s7JeEm"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("height")),
        Some(&serde_json::json!(720))
    );
}

#[test]
fn krasview_native_extractor_maps_player_json_and_open_graph() {
    let extractor = KrasViewExtractor::new(ExtractorDescriptor::new(
        "KrasViewIE",
        "KrasView",
        r"https?://krasview\.ru/(?:video|embed)/(?P<id>\d+)",
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta property="og:title" content="Native KrasView title">
                <meta property="og:description" content="Native description">
                <meta property="og:image" content="https://cdn.example/kras.jpg">
                <meta property="video:width" content="640">
                <meta property="video:height" content="360">
                <script>video_Init({"url":"https://cdn.example/kras.mp4",
                    "image":"https://cdn.example/player.jpg","duration":27})</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://krasview.ru/video/512228", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("512228"));
    assert_eq!(result.get_str("title"), Some("Native KrasView title"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/player.jpg")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(27)));
    assert_eq!(result.get("width"), Some(&serde_json::json!(640)));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/kras.mp4"));
}

#[test]
fn ku6_native_extractor_maps_page_title_and_api_media() {
    let extractor = Ku6Extractor::new(ExtractorDescriptor::new(
        "Ku6IE",
        "Ku6",
        r"https?://v\.ku6\.com/show/(?P<id>[a-zA-Z0-9\-\_]+)(?:\.)*html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "fetchVideo4Player/JG-8yS14xzBr4bCn1pu0xw".to_owned(),
                br#"{"data":{"f":"http://cdn.example/video.f4v"}}"#.to_vec(),
            ),
            (
                "v.ku6.com/show/JG-8yS14xzBr4bCn1pu0xw".to_owned(),
                br#"<h1 title="techniques test">techniques test</h1>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://v.ku6.com/show/JG-8yS14xzBr4bCn1pu0xw...html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("JG-8yS14xzBr4bCn1pu0xw"));
    assert_eq!(result.get_str("title"), Some("techniques test"));
    assert_eq!(result.get_str("url"), Some("http://cdn.example/video.f4v"));
    assert_eq!(result.get_str("ext"), Some("f4v"));
}

#[test]
fn graspop_native_extractor_maps_api_hls_and_poster() {
    let extractor = GraspopExtractor::new(ExtractorDescriptor::new(
        "GraspopIE",
        "Graspop",
        r"https?://vod\.graspop\.be/[a-z]{2}/(?P<id>\d+)/",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
                "name":"Thy Art Is Murder",
                "source":{
                    "assetUri":"https://cdn.example/festival/101556/master.m3u8",
                    "poster":"https://cdn.example/poster.jpg"
                }
            }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://vod.graspop.be/fr/101556/thy-art-is-murder-concert/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("101556"));
    assert_eq!(result.get_str("title"), Some("Thy Art Is Murder"));
    assert_eq!(
        result.get_str("url"),
        Some("http://cdn.example/festival/101556/master.m3u8")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/poster.jpg")
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
fn screenrec_native_extractor_maps_player_hls_and_open_graph() {
    let extractor = ScreenRecExtractor::new(ExtractorDescriptor::new(
        "ScreenRecIE",
        "ScreenRec",
        r"https?://(?:www\.)?screenrec\.com/share/(?P<id>\w{10})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html>
                <meta property="og:title" content="Screen recording">
                <meta property="og:description" content="Recorded with ScreenRec">
                <meta property="og:image" content="https://cdn.example/thumb.gif">
                <script>customUrl: "https://cdn.example/recording.m3u8"</script>
            </html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://screenrec.com/share/DasLtbknYo", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("DasLtbknYo"));
    assert_eq!(result.get_str("title"), Some("Screen recording"));
    assert_eq!(
        result.get_str("description"),
        Some("Recorded with ScreenRec")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/thumb.gif")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/recording.m3u8")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}

#[test]
fn matchtv_native_extractor_supports_on_air_and_iframe_urls() {
    let extractor = MatchTvExtractor::new(ExtractorDescriptor::with_valid_urls(
        "MatchTVIE",
        "MatchTV",
        vec![
            r"https?://matchtv\.ru/on-air/?(?:$|[?#])".to_owned(),
            r"https?://video\.matchtv\.ru/iframe/channel/106/?(?:$|[?#])".to_owned(),
        ],
        true,
    ))
    .unwrap();
    assert!(extractor.suitable("http://matchtv.ru/on-air/"));
    assert!(extractor.suitable("https://video.matchtv.ru/iframe/channel/106"));
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div data-config="config=https://stream.example/feed/channel106?token=1"></div>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://video.matchtv.ru/iframe/channel/106", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("matchtv-live"));
    assert_eq!(result.get_str("live_status"), Some("is_live"));
    assert_eq!(
        result.get_str("url"),
        Some("https://stream.example/media/channel106.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("is_live")),
        Some(&serde_json::json!(true))
    );
}

#[test]
fn hrefli_native_extractor_returns_percent_decoded_redirect() {
    let extractor = HrefLiRedirectExtractor::new(ExtractorDescriptor::new(
        "HrefLiRedirectIE",
        "href.li",
        r"https?://href\.li/\?(?P<url>.+)",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://href.li/?https%3A%2F%2Fexample.com%2Fwatch%3Fv%3Dabc",
            &ExtractionContext::native(),
        )
        .unwrap();
    assert_eq!(
        result,
        ExtractorResult::Redirect {
            url: "https://example.com/watch?v=abc".to_owned(),
            ie_key: None,
        }
    );
    assert_eq!(
        result.into_info_dict().get_str("url"),
        Some("https://example.com/watch?v=abc")
    );
}

#[test]
fn streamable_native_extractor_maps_ajax_files_and_metadata() {
    let extractor = StreamableExtractor::new(ExtractorDescriptor::new(
        "StreamableIE",
        "Streamable",
        r"https?://streamable\.com/(?:[es]/)?(?P<id>\w+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"{
                "status":2,
                "reddit_title":"Native Streamable title",
                "title":"Fallback title",
                "description":"Native description",
                "thumbnail_url":"//cdn.example/thumb.jpg",
                "owner":{"user_name":"native-user"},
                "date_added":1454964157.35115,
                "duration":61.516,
                "plays":42,
                "files":{
                    "mp4":{"url":"//cdn.example/video.mp4","width":1280,"height":720,"size":1234,"framerate":30,"bitrate":1500,"input_metadata":{"video_codec_name":"h264","audio_codec_name":"aac"}},
                    "webm":{"url":"https://cdn.example/video.webm","height":360}
                }
            }"#
            .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://streamable.com/e/demo1", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("demo1"));
    assert_eq!(result.get_str("title"), Some("Native Streamable title"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/video.mp4"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/thumb.jpg")
    );
    assert_eq!(result.get_str("uploader"), Some("native-user"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(61.516)));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(42)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("vbr")),
        Some(&serde_json::json!(1.5))
    );
}

#[test]
fn newgrounds_native_extractor_maps_embed_media_and_html_metadata() {
    let extractor = NewgroundsExtractor::new(ExtractorDescriptor::new(
            "NewgroundsIE",
            "Newgrounds",
            r"https?://(?:www\.)?newgrounds\.com/(?:audio/listen|portal/view)/(?P<id>\d+)(?:/format/flash)?",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<html>
                <head>
                    <title>Native Newgrounds title - Newgrounds</title>
                    <meta property="og:image" content="https://cdn.example/thumb.png">
                    <meta itemprop="uploadDate" content="2013-09-11">
                </head>
                <body>
                    <div id="author_comments"><p>Native <b>description</b></p></div>
                    <h4>Native author</h4><em>Author</em>
                    <h2 class="rated-m">Mature</h2>
                    <dl><dt>Views</dt><dd>1,234</dd></dl>
                    <script>
                        embedController([{"url":"//cdn.example/audio.mp3","description":"Audio File"}]);
                    </script>
                    <script>"duration":143</script>
                </body>
            </html>"#
                .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.newgrounds.com/audio/listen/549479", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("549479"));
    assert_eq!(result.get_str("title"), Some("Native Newgrounds title"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/audio.mp3"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get_str("uploader"), Some("Native author"));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1_378_857_600i64))
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(143.0)));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(17)));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(1234)));
    assert_eq!(result.get_str("description"), Some("Native description"));
}

#[test]
fn newgrounds_native_extractor_reads_json_source_fallback() {
    let extractor = NewgroundsExtractor::new(ExtractorDescriptor::new(
            "NewgroundsIE",
            "Newgrounds",
            r"https?://(?:www\.)?newgrounds\.com/(?:audio/listen|portal/view)/(?P<id>\d+)(?:/format/flash)?",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "portal/view/123".to_owned(),
                    br#"<html><head><title>Fallback title - Newgrounds</title></head>
                        <body><meta itemprop="datePublished" content="2020-01-02T03:04:05Z"></body></html>"#
                        .to_vec(),
                ),
                (
                    "portal/video/123".to_owned(),
                    br#"{"author":"JSON author","sources":{
                        "360p":[{"src":"https://cdn.example/360.mp4"}],
                        "720p":{"main":{"url":"https://cdn.example/720.mp4"}}
                    }}"#
                    .to_vec(),
                ),
            ],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.newgrounds.com/portal/view/123", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("title"), Some("Fallback title"));
    assert_eq!(result.get_str("uploader"), Some("JSON author"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("quality")),
        Some(&serde_json::json!(720))
    );
}
