#[test]
fn jwplatform_native_extractor_maps_json_sources_and_captions() {
    let extractor = JwPlatformExtractor::new(ExtractorDescriptor::new(
            "JWPlatformIE",
            "JWPlatform",
            r#"(?:https?://(?:content\.jwplatform|cdn\.jwplayer)\.com/(?:(?:feed|player|thumb|preview|manifest)s|jw6|v2/media)/|jwplatform:)(?P<id>[a-zA-Z0-9]{8})"#,
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"{
                "playlist":[{
                    "mediaid":"nPripu9l",
                    "title":"Big Buck Bunny Trailer",
                    "description":"<p>A short animated film.</p>",
                    "image":"https://cdn.example/poster.jpg",
                    "pubdate":1227796140,
                    "duration":32,
                    "sources":[
                        {"file":"https://cdn.example/video.mp4","label":"720p","width":1280,"height":720,"bitrate":500000},
                        {"file":"https://cdn.example/video.m3u8","type":"hls"}
                    ],
                    "tracks":[{"kind":"captions","label":"en","file":"https://cdn.example/captions.vtt"}]
                }]
            }"#
            .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("jwplatform:nPripu9l", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("nPripu9l"));
    assert_eq!(result.get_str("title"), Some("Big Buck Bunny Trailer"));
    assert_eq!(
        result.get_str("description"),
        Some("A short animated film.")
    );
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1227796140))
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(32.0)));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/video.mp4"));
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
        Some(&serde_json::json!("https://cdn.example/captions.vtt"))
    );
}

#[test]
fn jwplatform_wrappers_return_native_redirects() {
    let bundesliga = BundesligaExtractor::new(ExtractorDescriptor::new(
            "BundesligaIE",
            "Bundesliga",
            r"https?://(?:www\.)?bundesliga\.com/[a-z]{2}/bundesliga/videos(?:/[^?]+)?\?vid=(?P<id>[a-zA-Z0-9]{8})",
            true,
        ))
        .unwrap();
    assert_eq!(
        bundesliga
            .extract_with_context(
                "https://www.bundesliga.com/en/bundesliga/videos?vid=bhhHkKyN",
                &ExtractionContext::native(),
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "jwplatform:bhhHkKyN".to_owned(),
            ie_key: Some("JWPlatform".to_owned()),
        }
    );

    let outside = OutsideTvExtractor::new(ExtractorDescriptor::new(
            "OutsideTVIE",
            "OutsideTV",
            r"https?://(?:www\.)?outsidetv\.com/(?:[^/]+/)*?play/[a-zA-Z0-9]{8}/\d+/\d+/(?P<id>[a-zA-Z0-9]{8})",
            true,
        ))
        .unwrap();
    assert_eq!(
        outside
            .extract_with_context(
                "http://www.outsidetv.com/category/snow/play/ZjQYboH6/1/10/Hdg0jukV/4",
                &ExtractionContext::native(),
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "jwplatform:Hdg0jukV".to_owned(),
            ie_key: Some("JWPlatform".to_owned()),
        }
    );

    let teaching = TeachingChannelExtractor::new(ExtractorDescriptor::new(
        "TeachingChannelIE",
        "TeachingChannel",
        r"https?://(?:www\.)?teachingchannel\.org/videos?/(?P<id>[^/?&#]+)",
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<div id="jw-video-player-3swwlzkT"></div>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    assert_eq!(
        teaching
            .extract_with_context(
                "https://www.teachingchannel.org/videos/teacher-teaming-evolution",
                &context,
            )
            .unwrap(),
        ExtractorResult::Redirect {
            url: "jwplatform:3swwlzkT".to_owned(),
            ie_key: Some("JWPlatform".to_owned()),
        }
    );
}

#[test]
fn atscale_native_extractor_expands_data_url_video_playlist() {
    let extractor = AtScaleConfEventExtractor::new(ExtractorDescriptor::new(
        "AtScaleConfEventIE",
        "AtScaleConfEvent",
        r"https?://(?:www\.)?atscaleconference\.com/events/(?P<id>[^/&$?]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "events/data-scale-spring-2022".to_owned(),
                br#"<meta property="og:title" content="Data @Scale Spring 2022">
                        <meta property="og:description" content="Conference description">
                        <a data-url="https://atscaleconference.com/videos/one"></a>
                        <a data-url="https://www.atscaleconference.com/videos/two"></a>"#
                    .to_vec(),
            ),
            (
                "videos/one".to_owned(),
                br#"<meta property="og:title" content="Opening keynote">
                        <meta property="og:video" content="https://cdn.example/one.mp4">"#
                    .to_vec(),
            ),
            (
                "videos/two".to_owned(),
                br#"<meta property="og:title" content="Closing keynote">
                        <meta property="og:video" content="https://cdn.example/two.mp4">"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://atscaleconference.com/events/data-scale-spring-2022/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("playlist"));
    assert_eq!(result.get_str("id"), Some("data-scale-spring-2022"));
    assert_eq!(
        result.get_str("description"),
        Some("Conference description")
    );
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get("title"),
        Some(&serde_json::json!("Opening keynote"))
    );
    assert_eq!(
        entries[1].get("url"),
        Some(&serde_json::json!("https://cdn.example/two.mp4"))
    );
}

#[test]
fn nzz_native_extractor_parses_embedded_jwplayer_playlist() {
    let extractor = NzzExtractor::new(ExtractorDescriptor::new(
        "NZZIE",
        "NZZ",
        r"https?://(?:www\.)?nzz\.ch/(?:[^/]+/)*[^/?#]+-ld\.(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html>
                <meta property="og:title" content="NZZ article">
                <script data-hid="jw-video-jw-0">
                    var settings = {
                        "playlist":[
                            {"mediaid":"first","title":"First story",
                             "sources":[{"file":"https://cdn.example/first.m3u8","type":"hls"}]},
                            {"mediaid":"second","title":"Second story",
                             "sources":[{"file":"https://cdn.example/second.mp4","label":"720p",
                                         "width":1280,"height":720}]}
                        ]
                    };
                </script>
            </html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
            .extract_with_context(
                "https://www.nzz.ch/video/nzz-standpunkte/cvp-auf-der-suche-nach-dem-mass-der-mitte-ld.1368112",
                &context,
            )
            .unwrap()
            .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("playlist"));
    assert_eq!(result.get_str("id"), Some("1368112"));
    assert_eq!(result.get_str("title"), Some("NZZ article"));
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get("id"), Some(&serde_json::json!("first")));
    assert_eq!(
        entries[0]
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        entries[1].get("title"),
        Some(&serde_json::json!("Second story"))
    );
}

#[test]
fn behindkink_native_extractor_maps_html5_video_and_date() {
    let extractor = BehindKinkExtractor::new(ExtractorDescriptor::new(
            "BehindKinkIE",
            "BehindKink",
            r"https?://(?:www\.)?behindkink\.com/(?P<year>[0-9]{4})/(?P<month>[0-9]{2})/(?P<day>[0-9]{2})/(?P<id>[^/#?_]+)",
            false,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta property="og:title" content="What are you passionate about - Marley Blaze">
                <meta property="og:image" content="https://cdn.example/blaze.jpg">
                <meta property="og:description" content="Native description">
                <video><source src="https://cdn.example/37127_master.mp4"></video>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.behindkink.com/2014/12/05/what-are-you-passionate-about-marley-blaze/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("37127"));
    assert_eq!(
        result.get_str("display_id"),
        Some("what-are-you-passionate-about-marley-blaze")
    );
    assert_eq!(result.get_str("upload_date"), Some("20141205"));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(18)));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/37127_master.mp4")
    );
}

#[test]
fn historicfilms_native_extractor_builds_media_url_and_metadata() {
    let extractor = HistoricFilmsExtractor::new(ExtractorDescriptor::new(
        "HistoricFilmsIE",
        "HistoricFilms",
        r"https?://(?:www\.)?historicfilms\.com/(?:tapes/|play)(?P<id>\d+)",
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<span class="tapeId">AB-12</span>
                <meta property="og:title" content="Historic Films: GP-7">
                <meta property="og:description" content="Native archive description">
                <meta name="thumbnailUrl" content="https://cdn.example/4728.jpg">
                <meta name="duration" content="2096">"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://www.historicfilms.com/tapes/4728", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("4728"));
    assert_eq!(
        result.get_str("url"),
        Some("http://www.historicfilms.com/video/AB-12_4728_web.mov")
    );
    assert_eq!(result.get_str("ext"), Some("mov"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(2096.0)));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/4728.jpg")
    );
}

#[test]
fn oneplace_native_extractor_maps_audio_player_fields() {
    let extractor = OnePlacePodcastExtractor::new(ExtractorDescriptor::new(
        "OnePlacePodcastIE",
        "OnePlacePodcast",
        r"https?://www\.oneplace\.com/[\w]+/[^/]+/listen/[\w-]+-(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta property="og:title" content="Living in the Last Days Part 2">
                <div id="player" data-media-url="https://cdn.example/958461.mp3"></div>
                <div class="epDesc">Native episode description</div>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
            .extract_with_context(
                "https://www.oneplace.com/ministries/a-daily-walk/listen/living-in-the-last-days-part-2-958461.html",
                &context,
            )
            .unwrap()
            .into_info_dict();

    assert_eq!(result.get_str("id"), Some("958461"));
    assert_eq!(
        result.get_str("title"),
        Some("Living in the Last Days Part 2")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native episode description")
    );
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get_str("vcodec"), Some("none"));
}

#[test]
fn megaphone_native_extractor_maps_embedded_episode_audio() {
    let extractor = MegaphoneExtractor::new(ExtractorDescriptor::new(
        "MegaphoneIE",
        "megaphone.fm",
        r"https?://player\.megaphone\.fm/(?P<id>[A-Z0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br##"<meta property="audio:title" content="#97 What Kind Of Idiot Gets Phished?">
                <meta property="audio:artist" content="Reply All">
                <meta property="og:image" content="https://cdn.example/show.png">
                <script>var episode = {
                    "mediaUrl":"//cdn.example/GLT9749789991.mp3",
                    "duration":1998.36
                };</script>"##
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://player.megaphone.fm/GLT9749789991", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("GLT9749789991"));
    assert_eq!(
        result.get_str("title"),
        Some("#97 What Kind Of Idiot Gets Phished?")
    );
    assert_eq!(result.get("duration"), Some(&serde_json::json!(1998.36)));
    assert_eq!(
        result.get("creators"),
        Some(&serde_json::json!(["Reply All"]))
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/GLT9749789991.mp3")
    );
}

#[test]
fn hypem_native_extractor_maps_page_track_and_source_api() {
    let extractor = HypemExtractor::new(ExtractorDescriptor::new(
        "HypemIE",
        "Hypem",
        r"https?://(?:www\.)?hypem\.com/track/(?P<id>[0-9a-z]{5})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "serve/source/1v6ga/source-key".to_owned(),
                br#"{"url":"https://cdn.example/tame.mp3"}"#.to_vec(),
            ),
            (
                "hypem.com/track/1v6ga".to_owned(),
                br#"<script type="application/json" id="displayList-data">
                        {"tracks":[{"id":"1v6ga","key":"source-key","song":"Tame",
                                   "artist":"BODYWORK","time":180,"ts":1371810457}]}
                    </script>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://hypem.com/track/1v6ga/BODYWORK+-+TAME", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1v6ga"));
    assert_eq!(result.get_str("title"), Some("Tame"));
    assert_eq!(result.get_str("uploader"), Some("BODYWORK"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(180)));
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/tame.mp3"));
}
