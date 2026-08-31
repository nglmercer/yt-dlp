#[test]
fn qingting_native_extractor_maps_embedded_program_store() {
    let extractor = QingTingExtractor::new(ExtractorDescriptor::new(
            "QingTingIE",
            "QingTing",
            r"https?://(?:www\.|m\.)?(?:qingting\.fm|qtfm\.cn)/v?channels/(?P<channel>\d+)/programs/(?P<id>\d+)",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<script>window.__initStores = {
                "ProgramStore":{
                    "programInfo":{"title":"Native program","audioUrl":"https://cdn.example/program.m4a"},
                    "channelInfo":{"title":"Native channel"},
                    "podcasterInfo":{"podcaster":{"nickname":"Native host"}}
                }
            };</script>"#
                .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.qingting.fm/channels/378005/programs/22257411/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("22257411"));
    assert_eq!(result.get_str("channel_id"), Some("378005"));
    assert_eq!(result.get_str("channel"), Some("Native channel"));
    assert_eq!(result.get_str("uploader"), Some("Native host"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/program.m4a")
    );
    assert_eq!(result.get_str("acodec"), Some("m4a"));
}

#[test]
fn skyline_native_extractor_maps_live_hls_and_open_graph() {
    let extractor = SkylineWebcamsExtractor::new(ExtractorDescriptor::new(
        "SkylineWebcamsIE",
        "SkylineWebcams",
        r"https?://(?:www\.)?skylinewebcams\.com/[^/]+/webcam/(?:[^/]+/)+(?P<id>[^/]+)\.html",
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body:
            br#"<meta property="og:title" content="Live Webcam Rome">
                <meta property="og:description" content="Native webcam">
                <script>const config = {source: "https://cdn.example/rome.m3u8?token=1"};</script>"#
                .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.skylinewebcams.com/it/webcam/italia/lazio/roma/rome.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("rome"));
    assert_eq!(result.get_str("title"), Some("Live Webcam Rome"));
    assert_eq!(result.get_str("live_status"), Some("is_live"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/rome.m3u8?token=1")
    );
}

#[test]
fn webcamerapl_native_extractor_decodes_rot13_live_hls() {
    let extractor = WebcameraplExtractor::new(ExtractorDescriptor::new(
        "WebcameraplIE",
        "Webcamerapl",
        r"https?://(?P<id>[\w-]+)\.webcamera\.pl",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<h1>WIDOK NA PLAC ZAMKOWY</h1>
                <div data-src="uggcf://pqa.rknzcyr.pbz/yvir/znfgre.z3h8"></div>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://warszawa-plac-zamkowy.webcamera.pl/", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("warszawa-plac-zamkowy"));
    assert_eq!(result.get_str("title"), Some("WIDOK NA PLAC ZAMKOWY"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example.com/live/master.m3u8")
    );
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
}

#[test]
fn alibaba_native_extractor_maps_embedded_product_video() {
    let extractor = AlibabaExtractor::new(ExtractorDescriptor::new(
        "AlibabaIE",
        "Alibaba",
        r"https?://(?:www\.)?alibaba\.com/product-detail/[\w-]+_(?P<id>\d+)\.html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>window.detailData = {
                "globalData":{"product":{
                    "subject":"Native product",
                    "mediaItems":[{
                        "type":"video","videoId":6000280444270,
                        "videoUrl":"https://cdn.example/product.mp4",
                        "definition":"720p","duration":30,
                        "videoCoverUrl":"https://cdn.example/product.jpg",
                        "bitrate":500000,"width":1280,"height":720,"length":12345
                    }]
                }}
            };</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.alibaba.com/product-detail/Native-Product_1601271126969.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("6000280444270"));
    assert_eq!(result.get_str("display_id"), Some("1601271126969"));
    assert_eq!(result.get_str("title"), Some("Native product"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(30.0)));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/product.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/product.mp4")
    );
}

#[test]
fn c56_native_extractor_maps_video_api_files() {
    let extractor = C56Extractor::new(ExtractorDescriptor::new(
            "C56IE",
            "56.com",
            r"https?://(?:(?:www|player)\.)?56\.com/(?:.+?/)?(?:v_|(?:play_album.+-))(?P<textid>.+?)\.(?:html|swf)",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "vxml.56.com/json/OTM0NDA3MTY".to_owned(),
                    br#"{"info":{
                        "vid":"93440716","Subject":"Native 56 video","duration":283813,
                        "bimg":"https://cdn.example/cover.jpg",
                        "rfiles":[{"type":"flv","filesize":"12345","url":"https://cdn.example/video.flv"}]
                    }}"#
                    .to_vec(),
                ),
                (
                    "56.com/u39/v_OTM0NDA3MTY".to_owned(),
                    br#"<html><title>Native 56 page</title></html>"#.to_vec(),
                ),
            ],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://www.56.com/u39/v_OTM0NDA3MTY.html", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("93440716"));
    assert_eq!(result.get_str("title"), Some("Native 56 video"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(283.813)));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/cover.jpg")
    );
    assert_eq!(result.get_str("ext"), Some("flv"));
}

#[test]
fn tass_native_extractor_maps_embedded_http_mp4_sources() {
    let extractor = TassExtractor::new(ExtractorDescriptor::new(
        "TassIE",
        "Tass",
        r"https?://(?:tass\.ru|itar-tass\.com)/[^/]+/(?P<id>\d+)",
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta property="og:title" content="Native TASS title">
                <meta property="og:description" content="Native TASS description">
                <meta property="og:image" content="https://cdn.example/tass.jpg">
                <script>var player = {sources: [
                    {"file":"https://cdn.example/tass-sd.mp4","label":"sd"},
                    {"file":"https://cdn.example/tass-hd.mp4","label":"hd"},
                    {"file":"https://cdn.example/tass.flv","label":"flv"}
                ]};</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://tass.ru/obschestvo/1586870", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1586870"));
    assert_eq!(result.get_str("title"), Some("Native TASS title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native TASS description")
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
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.get(1))
            .and_then(|format| format.get("quality")),
        Some(&serde_json::json!(1))
    );
}

#[test]
fn photobucket_native_extractor_maps_shared_metadata_and_file_code() {
    let extractor = PhotobucketExtractor::new(ExtractorDescriptor::new(
            "PhotobucketIE",
            "Photobucket",
            r"https?://(?:[a-z0-9]+\.)?photobucket\.com/.*(([?\&]current=)|_)(?P<id>.*)\.(?P<ext>(flv)|(mp4))",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<script>Pb.Data.Shared.put(Pb.Data.Shared.MEDIA, {
                "username":"rachaneronas",
                "creationDate":1367669341,
                "title":"Native Photobucket video",
                "thumbUrl":"https://cdn.example/thumb.jpg",
                "linkcodes":{"html":"<a href=\"https://cdn.example/player?file=https%3A%2F%2Fcdn.example%2Fvideo.mp4\">video</a>"}
            });</script>"#
            .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://media.photobucket.com/user/rachaneronas/media/Tired_zpsc0c3b9fa.mp4.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("zpsc0c3b9fa"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get_str("uploader"), Some("rachaneronas"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/video.mp4"));
}

#[test]
fn nobelprize_native_extractor_maps_jsonld_media_metadata() {
    let extractor = NobelPrizeExtractor::new(ExtractorDescriptor::new(
        "NobelPrizeIE",
        "NobelPrize",
        r"https?://(?:(?:mediaplayer|www)\.)?nobelprize\.org/mediaplayer/",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta name="caption" content="Native Nobel Lecture">
                <script type="application/ld+json">{
                    "name":"JSON-LD title",
                    "description":"Native Nobel description",
                    "contentUrl":"https://cdn.example/nobel.mp4",
                    "thumbnailUrl":"https://cdn.example/nobel.jpg",
                    "duration":"PT26M",
                    "uploadDate":"2017-09-08T12:00:00Z"
                }</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.nobelprize.org/mediaplayer/?id=2636", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2636"));
    assert_eq!(result.get_str("title"), Some("Native Nobel Lecture"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Nobel description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/nobel.jpg")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(1560.0)));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1504872000i64))
    );
}

#[test]
fn caltrans_native_extractor_maps_live_camera_hls() {
    let extractor = CaltransExtractor::new(ExtractorDescriptor::new(
        "CaltransIE",
        "Caltrans",
        r"https?://(?:[^/]+\.)?ca\.gov/vm/loc/[^/]+/(?P<id>[a-z0-9_]+)\.htm",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>
                routePlace = "US-50";
                locationName = "Sacramento";
                posterURL = "https://cdn.example/cam.jpg";
                videoStreamURL = "https://cdn.example/cam.m3u8";
            </script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://cwwp2.dot.ca.gov/vm/loc/d3/hwy50at24th.htm",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("hwy50at24th"));
    assert_eq!(result.get_str("title"), Some("US-50 : Sacramento"));
    assert_eq!(result.get_str("ext"), Some("ts"));
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/cam.m3u8"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/cam.jpg")
    );
}

#[test]
fn cozytv_native_extractor_maps_replay_api_and_hls() {
    let extractor = CozyTvExtractor::new(ExtractorDescriptor::new(
        "CozyTVIE",
        "CozyTV",
        r"https?://(?:www\.)?cozy\.tv/(?P<uploader>[^/]+)/replays/(?P<id>[^/$#&?]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
                "title":"Native Cozy replay",
                "user":"beardson",
                "date":"2021-11-19",
                "duration":7981
            }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://cozy.tv/beardson/replays/2021-11-19_1", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("beardson-2021-11-19_1"));
    assert_eq!(result.get_str("title"), Some("Native Cozy replay"));
    assert_eq!(result.get_str("uploader"), Some("beardson"));
    assert_eq!(result.get_str("upload_date"), Some("20211119"));
    assert_eq!(result.get("was_live"), Some(&serde_json::json!(true)));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(7981)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cozycdn.foxtrotstream.xyz/replays/beardson/2021-11-19_1/index.m3u8")
    );
}

#[test]
fn livestreamfails_native_extractor_maps_api_clip() {
    let extractor = LivestreamfailsExtractor::new(ExtractorDescriptor::new(
        "LivestreamfailsIE",
        "Livestreamfails",
        r"https?://(?:www\.)?livestreamfails\.com/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
                "sourceId":"ConcernedStreamer",
                "videoId":"abc123",
                "createdAt":"2022-06-26T12:49:45Z",
                "label":"Streamer jumps",
                "streamer":{"label":"paradeev1ch"},
                "imageId":"img1"
            }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://livestreamfails.com/clip-123", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("clip-123"));
    assert_eq!(result.get_str("display_id"), Some("ConcernedStreamer"));
    assert_eq!(result.get_str("title"), Some("Streamer jumps"));
    assert_eq!(result.get_str("creator"), Some("paradeev1ch"));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1656247785i64))
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://livestreamfails-video-prod.b-cdn.net/video/abc123")
    );
}

#[test]
fn masters_native_extractor_maps_tournament_hls_and_thumbnails() {
    let extractor = MastersExtractor::new(ExtractorDescriptor::new(
        "MastersIE",
        "Masters",
        r"https?://(?:www\.)?masters\.com/en_US/video/(?P<date>\d{4}-\d{2}-\d{2})/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
                "title":"Native Masters video",
                "media":{"m3u8":"https://cdn.example/master.m3u8"},
                "images":[{
                    "poster":"https://cdn.example/poster.jpg",
                    "thumbnail":"https://cdn.example/thumb.jpg"
                }]
            }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.masters.com/en_US/video/2024-04-14/final-round",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("final-round"));
    assert_eq!(result.get_str("title"), Some("Native Masters video"));
    assert_eq!(result.get_str("upload_date"), Some("20240414"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/master.m3u8")
    );
    assert_eq!(
        result
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn mir24_native_extractor_maps_article_player_source() {
    let extractor = Mir24TvExtractor::new(ExtractorDescriptor::new(
        "Mir24TvIE",
        "Mir24",
        r"https?://(?:www\.)?mir24\.tv/news/(?P<id>\d+)",
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<meta property="og:title" content="Native Mir24 title">
                <meta property="og:image" content="https://cdn.example/mir.jpg">
                <iframe src="https://mir24.tv/players/foo?source=https%3A%2F%2Fcdn.example%2Fmir.m3u8"></iframe>"#
                .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://mir24.tv/news/16635210/slug", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("16635210"));
    assert_eq!(result.get_str("title"), Some("Native Mir24 title"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/mir.jpg")
    );
    assert_eq!(result.get_str("url"), Some("https://cdn.example/mir.m3u8"));
}

#[test]
fn blogger_native_extractor_maps_video_config_streams() {
    let extractor = BloggerExtractor::new(ExtractorDescriptor::new(
        "BloggerIE",
        "blogger.com",
        r"https?://(?:www\.)?blogger\.com/video\.g\?token=(?P<id>.+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<script>var VIDEO_CONFIG = {
                "iframe_id":"BLOGGER-video-native-796",
                "thumbnail":"https://cdn.example/blogger.jpg",
                "streams":[
                    {"play_url":"https://cdn.example/video.mp4?mime=video/mp4&dur=76.068","format_id":"720p"},
                    {"play_url":"https://cdn.example/video.webm?mime=video/webm&dur=76.068","format_id":"webm"}
                ]
            };</script>"#
                .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.blogger.com/video.g?token=native-token",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("BLOGGER-video-native-796"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(76.068)));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/blogger.jpg")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}
