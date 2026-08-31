use super::*;
use yt_dlp_networking::{CookieJar, Request, RequestDirector, RequestError, Response};
use yt_dlp_networking::{ErrorKind, RequestHandler};

struct FakeHandler {
    body: Vec<u8>,
}

impl RequestHandler for FakeHandler {
    fn name(&self) -> &str {
        "extractor-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        Ok(Response::new(request.url(), 200, "OK", self.body.clone()))
    }
}

struct RoutedHandler {
    routes: Vec<(String, Vec<u8>)>,
}

impl RequestHandler for RoutedHandler {
    fn name(&self) -> &str {
        "extractor-route-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let body = self
            .routes
            .iter()
            .find(|(needle, _)| request.url().contains(needle))
            .map(|(_, body)| body.clone())
            .ok_or_else(|| {
                RequestError::new(
                    ErrorKind::Transport,
                    format!("no test route for {}", request.url()),
                )
            })?;
        Ok(Response::new(request.url(), 200, "OK", body))
    }
}

#[test]
fn registry_preserves_registration_order() {
    let mut registry = ExtractorRegistry::new();
    registry
        .register(
            DescriptorExtractor::new(ExtractorDescriptor::new(
                "first",
                "First",
                r"^https://example\.com/.*$",
                true,
            ))
            .unwrap(),
        )
        .unwrap();
    registry
        .register(
            DescriptorExtractor::new(ExtractorDescriptor::new(
                "second",
                "Second",
                r"^https://example\.com/video$",
                true,
            ))
            .unwrap(),
        )
        .unwrap();

    assert_eq!(registry.len(), 2);
    assert_eq!(
        registry
            .find("https://example.com/video")
            .unwrap()
            .descriptor()
            .key,
        "first"
    );
}

#[test]
fn unported_descriptor_is_explicitly_unsupported() {
    let extractor = DescriptorExtractor::new(ExtractorDescriptor::new(
        "test",
        "Test",
        r"^https://test\.example/",
        false,
    ))
    .unwrap();
    let error = extractor.extract("https://test.example/video").unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.contains("not ported"));
}

#[test]
fn duplicate_keys_and_invalid_patterns_are_rejected() {
    let mut registry = ExtractorRegistry::new();
    let first = DescriptorExtractor::new(ExtractorDescriptor::new(
        "same",
        "Same",
        r"^https://example\.com",
        true,
    ))
    .unwrap();
    registry.register(first).unwrap();
    let duplicate = DescriptorExtractor::new(ExtractorDescriptor::new(
        "same",
        "Same again",
        r"^https://other\.example",
        true,
    ))
    .unwrap();
    assert!(registry.register(duplicate).is_err());
    assert!(
        DescriptorExtractor::new(ExtractorDescriptor::new("broken", "Broken", "[", true,)).is_err()
    );
}

#[test]
fn generated_manifest_preserves_extractor_inventory_and_order() {
    let registry = ExtractorRegistry::generated().unwrap();

    // Refresh this snapshot whenever the source extractor registry is
    // intentionally regenerated.
    assert_eq!(registry.len(), 1_752);
    assert!(registry.native_matchable_count() > 1_000);
    assert!(registry.native_pattern_count() > 1_000);
    assert_eq!(registry.pattern_error_count(), 0);
    assert_eq!(
        registry.iter().last().unwrap().descriptor().key,
        "GenericIE"
    );
    assert_eq!(registry.native_implementation_count(), 93);
}

#[test]
fn generic_extractor_returns_stable_url_metadata() {
    let registry = ExtractorRegistry::generated().unwrap();
    let info = registry
        .extract("https://media.example.test/path/sample-video.MP4?token=1")
        .unwrap();
    assert_eq!(info.get("id"), Some(&serde_json::json!("sample-video")));
    assert_eq!(info.get("title"), Some(&serde_json::json!("sample-video")));
    assert_eq!(info.get("ext"), Some(&serde_json::json!("mp4")));
    assert_eq!(info.get("direct"), Some(&serde_json::json!(true)));
}

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

#[test]
fn radiode_native_extractor_maps_station_broadcast_and_streams() {
    let extractor = RadioDeExtractor::new(ExtractorDescriptor::new(
        "RadioDeIE",
        "radio.de",
        r"https?://(?P<id>.+?)\.(?:radio\.(?:de|at|fr|pt|es|pl|it)|rad\.io)",
        false,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>
                'components/station/stationService': {station: {
                    name: "NDR 2",
                    description: "Native radio description",
                    picture4Url: "https://cdn.example/ndr.png",
                    streamUrls: [{
                        streamUrl: "https://cdn.example/ndr.mp3",
                        streamContentFormat: "MP3",
                        bitRate: 128,
                        sampleRate: 44100
                    }]
                }}
            </script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://ndr2.radio.de/", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("ndr2"));
    assert_eq!(result.get_str("title"), Some("NDR 2"));
    assert_eq!(
        result.get_str("description"),
        Some("Native radio description")
    );
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get("is_live"), Some(&serde_json::json!(true)));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/ndr.mp3"));
}

#[test]
fn radiozet_native_extractor_maps_page_and_podcast_api() {
    let extractor = RadioZetPodcastExtractor::new(ExtractorDescriptor::new(
        "RadioZetPodcastIE",
        "RadioZetPodcast",
        r"https?://player\.radiozet\.pl\/Podcasty/.*?/(?P<id>.+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "player.radiozet.pl/Podcasty".to_owned(),
                br#"<div id="player" data-id="42154"></div>"#.to_vec(),
            ),
            (
                "api/podcasts/getPodcast".to_owned(),
                r#"{"data":[{
                        "title":"Native RadioZET episode",
                        "published_date":1592985480,
                        "player":{"stream":"https://cdn.example/radiozet.mp3","duration":83},
                        "program":{"desc":"Native podcast description","title":"Nie Ma Za Co",
                                  "image":{"original":"https://cdn.example/radiozet.png"}},
                        "presenter":[{"title":"Katarzyna Pakosińska"}]
                    }]}"#
                    .as_bytes()
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://player.radiozet.pl/Podcasty/Nie-Ma-Za-Co/Native-episode",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("42154"));
    assert_eq!(result.get_str("display_id"), Some("Native-episode"));
    assert_eq!(result.get_str("series"), Some("Nie Ma Za Co"));
    assert_eq!(result.get_str("creator"), Some("Katarzyna Pakosińska"));
    assert_eq!(
        result.get("release_timestamp"),
        Some(&serde_json::json!(1592985480i64))
    );
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/radiozet.mp3")
    );
}

#[test]
fn worldstar_native_extractor_maps_html5_media() {
    let extractor = WorldStarHipHopExtractor::new(ExtractorDescriptor::new(
            "WorldStarHipHopIE",
            "WorldStarHipHop",
            r"https?://(?:www|m)\.worldstar(?:candy|hiphop)\.com/(?:videos|android)/video\.php\?.*?\bv=(?P<id>[^&]+)",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta property="og:image" content="https://cdn.example/worldstar.jpg">
                <div class="content-heading"><h1>Native WorldStar title</h1></div>
                <video><source src="https://cdn.example/worldstar.mp4" type="video/mp4"></video>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.worldstarhiphop.com/videos/video.php?v=native123",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native123"));
    assert_eq!(result.get_str("title"), Some("Native WorldStar title"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/worldstar.mp4")
    );
}

#[test]
fn thisamericanlife_native_extractor_maps_archive_audio_metadata() {
    let extractor = ThisAmericanLifeExtractor::new(ExtractorDescriptor::new(
            "ThisAmericanLifeIE",
            "ThisAmericanLife",
            r"https?://(?:www\.)?thisamericanlife\.org/(?:radio-archives/episode/|play_full\.php\?play=)(?P<id>\d+)",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta name="twitter:title" content="487: Native Episode">
                <meta name="description" content="Native episode description">
                <meta property="og:image" content="https://cdn.example/tal.jpg">"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://www.thisamericanlife.org/radio-archives/episode/487/native",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("487"));
    assert_eq!(result.get_str("title"), Some("487: Native Episode"));
    assert_eq!(result.get_str("ext"), Some("m4a"));
    assert_eq!(result.get_str("protocol"), Some("m3u8_native"));
    assert_eq!(
        result.get_str("url"),
        Some("http://stream.thisamericanlife.org/487/stream/487_64k.m3u8")
    );
}

#[test]
fn academicearth_native_extractor_builds_url_playlist_entries() {
    let extractor = AcademicEarthCourseExtractor::new(ExtractorDescriptor::new(
        "AcademicEarthCourseIE",
        "AcademicEarth:Course",
        r"https?://(?:www\.)?academicearth\.org/playlists/(?P<id>[^?#/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<h1 class="playlist-name">Native Laws of Nature</h1>
                <p class="excerpt">Native course description</p>
                <ul>
                    <li class="lecture-preview"><a target="_blank" href="/lectures/one">One</a></li>
                    <li class="lecture-preview"><a target="_blank" href="https://cdn.example/two">Two</a></li>
                </ul>"#
                .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let extraction = extractor
        .extract_with_context(
            "http://academicearth.org/playlists/laws-of-nature/",
            &context,
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = extraction else {
        panic!("Academic Earth should return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("laws-of-nature"));
    assert_eq!(info.get_str("title"), Some("Native Laws of Nature"));
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get_str("url"),
        Some("http://academicearth.org/lectures/one")
    );
    assert_eq!(entries[1].get_str("url"), Some("https://cdn.example/two"));
    assert_eq!(entries[0].get_str("_type"), Some("url"));
}

#[test]
fn premiershiprugby_native_extractor_maps_article_hls_metadata() {
    let extractor = PremiershipRugbyExtractor::new(ExtractorDescriptor::new(
        "PremiershipRugbyIE",
        "PremiershipRugby",
        r"https?://(?:\w+\.)premiershiprugby\.(?:com)/watch/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"data":{"article":{
                "heroMedia":{
                    "title":"Native Full Match",
                    "content":{
                        "sourceSystemId":"0_native",
                        "videoLink":"https://cdn.example/premiership.m3u8",
                        "videoThumbnail":"https://cdn.example/premiership.jpg",
                        "metadata":{"msDuration":6093000}
                    }
                },
                "tags":["video"],
                "categories":[{"text":"Full Match"},{"text":"Harlequins"}]
            }}}"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.premiershiprugby.com/watch/native-full-match",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("0_native"));
    assert_eq!(result.get_str("display_id"), Some("native-full-match"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(6093.0)));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result
            .get("categories")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/premiership.m3u8")
    );
}

#[test]
fn matchitv_native_extractor_maps_next_data_and_hls() {
    let extractor = MatchiTvExtractor::new(ExtractorDescriptor::new(
        "MatchiTVIE",
        "MatchiTV",
        r"https?://(?:www\.)?matchi\.tv/watch/?\?(?:[^#]+&)?s=(?P<id>[0-9a-zA-Z]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script id="__NEXT_DATA__" type="application/json">{
                "props":{"pageProps":{"loadedMedia":{
                    "courtDescription":"Court 2",
                    "startDateTime":"2024-07-13T18:32:24"
                }}}
            }</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://matchi.tv/watch?s=0euhjzrxsjm", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("0euhjzrxsjm"));
    assert_eq!(result.get_str("title"), Some("Court 2 2024-07-13T18:32:24"));
    assert_eq!(result.get_str("upload_date"), Some("20240713"));
    assert_eq!(
        result.get_str("url"),
        Some("https://streams.padelgo.tv/v2/streams/m3u8/0euhjzrxsjm/anonymous/playlist.m3u8")
    );
}

#[test]
fn sztvhu_native_extractor_maps_vod_page_fields() {
    let extractor = SztvHuExtractor::new(ExtractorDescriptor::new(
        "SztvHuIE",
        "SztvHu",
        r"https?://(?:(?:www\.)?sztv\.hu|www\.tvszombathely\.hu)/(?:[^/]+)/.+-(?P<id>[0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta name="title" content="Native SZTV title - News - SZTV">
                <meta name="description" content="Native SZTV description">
                <meta property="og:image" content="https://cdn.example/sztv.jpg">
                <script>var player = {file: "http://media.sztv.hu:native.mp4",};</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://sztv.hu/hirek/native-sztv-title-20130909", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("20130909"));
    assert_eq!(result.get_str("title"), Some("Native SZTV title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native SZTV description")
    );
    assert_eq!(
        result.get_str("url"),
        Some("http://media.sztv.hu/vod/native.mp4")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}

#[test]
fn arnes_native_extractor_maps_public_video_api() {
    let extractor = ArnesExtractor::new(ExtractorDescriptor::new(
            "ArnesIE",
            "video.arnes.si",
            r"https?://video\.arnes\.si/(?:[a-z]{2}/)?(?:watch|embed|api/(?:asset|public/video))/(?P<id>[0-9a-zA-Z]{12})",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"data":{
                "title":"Native Arnes lecture",
                "description":"Native Arnes description",
                "license":"PRIVATE",
                "author":"Polona Oblak",
                "creationTime":"2020-03-24T10:08:45Z",
                "thumbnailUrl":"/media/thumb.jpg",
                "duration":596750,
                "views":17,
                "hashtags":["linearna_algebra"],
                "channel":{"url":"q6pc04hw24cj","name":"Polona Oblak"},
                "media":[{"url":"/api/asset/a1qrWTOQfVoU/play.mp4",
                          "format":"FORMAT_720P","formatTranslation":"720p",
                          "width":1280,"height":720}]
            }}"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://video.arnes.si/watch/a1qrWTOQfVoU?t=10", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("a1qrWTOQfVoU"));
    assert_eq!(result.get_str("title"), Some("Native Arnes lecture"));
    assert_eq!(result.get_str("format"), None);
    assert_eq!(result.get_str("creator"), Some("Polona Oblak"));
    assert_eq!(result.get_str("channel_id"), Some("q6pc04hw24cj"));
    assert_eq!(result.get_str("upload_date"), None);
    assert_eq!(result.get("duration"), Some(&serde_json::json!(596.75)));
    assert_eq!(result.get("start_time"), Some(&serde_json::json!(10)));
    assert_eq!(
        result.get_str("url"),
        Some("https://video.arnes.si/api/asset/a1qrWTOQfVoU/play.mp4")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}

#[test]
fn cjsw_native_extractor_maps_episode_audio_page() {
    let extractor = CjswExtractor::new(ExtractorDescriptor::new(
        "CJSWIE",
        "CJSW",
        r"https?://(?:www\.)?cjsw\.com/program/(?P<program>[^/]+)/episode/(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<h1 class="episode-header__title">Freshly Squeezed - Native Episode</h1>
                <button data-audio-src="https://cdn.example/91d9f016-a2e7-46c5-8dcb-7cbcd7437c41.mp3"></button>
                <p>Native CJSW description</p>
                <div data-showname="Freshly Squeezed"></div>"#
                .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://cjsw.com/program/freshly-squeezed/episode/20170620",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("91d9f016-a2e7-46c5-8dcb-7cbcd7437c41")
    );
    assert_eq!(
        result.get_str("title"),
        Some("Freshly Squeezed - Native Episode")
    );
    assert_eq!(result.get_str("series"), Some("Freshly Squeezed"));
    assert_eq!(result.get_str("episode_id"), Some("20170620"));
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/91d9f016-a2e7-46c5-8dcb-7cbcd7437c41.mp3")
    );
}

#[test]
fn daystar_native_extractor_maps_lightcast_config_hls() {
    let extractor = DaystarClipExtractor::new(ExtractorDescriptor::new(
        "DaystarClipIE",
        "daystar:clip",
        r"https?://player\.daystar\.tv/(?P<id>\w+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "player.daystar.tv/0MTO2ITM".to_owned(),
                    br#"<meta property="og:title" content="Native Daystar clip">
                        <meta property="og:description" content="Native Daystar description">
                        <iframe src="https://www.lightcast.com/embed/player.php?id=0MTO2ITM"></iframe>"#
                        .to_vec(),
                ),
                (
                    "config2.php".to_owned(),
                    br#"sources: [
                        {"file":"https://cdn.example/daystar.m3u8","type":"m3u8"}
                    ],
                    image: "https://cdn.example/daystar.jpg""#
                    .to_vec(),
                ),
            ],
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://player.daystar.tv/0MTO2ITM", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("0MTO2ITM"));
    assert_eq!(result.get_str("title"), Some("Native Daystar clip"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/daystar.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/daystar.m3u8")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}

#[test]
fn dctp_native_extractor_maps_versioned_rest_api_and_formats() {
    let extractor = DctpTvExtractor::new(ExtractorDescriptor::new(
        "DctpTvIE",
        "DctpTv",
        r"https?://(?:www\.)?dctp\.tv/(?:#/)?filme/(?P<id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "dctp-ivms2-restapi.s3.amazonaws.com/version.json".to_owned(),
                br#"{"version_name":"v1"}"#.to_vec(),
            ),
            (
                "/v1/restapi/slugs/native-film.json".to_owned(),
                br#"{"object_id":95}"#.to_vec(),
            ),
            (
                "/v1/restapi/media/95.json".to_owned(),
                br#"{
                        "uuid":"95eaa4f33dad413aa17b4ee613cccc6c",
                        "title":"Native DCTP film",
                        "subtitle":"Native subtitle",
                        "description":"Native DCTP description",
                        "created":"2011-04-07T10:32:02Z",
                        "duration_in_ms":71240,
                        "is_wide":true,
                        "images":[{"url":"https://cdn.example/dctp.jpg","width":640,"height":360}]
                    }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://www.dctp.tv/filme/native-film/", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("95eaa4f33dad413aa17b4ee613cccc6c")
    );
    assert_eq!(result.get_str("display_id"), Some("native-film"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(71.24)));
    assert_eq!(
        result.get_str("url"),
        Some(
            "https://cdn-segments.dctp.tv/95eaa4f33dad413aa17b4ee613cccc6c_dctp_0500_16x9.m4v/playlist.m3u8"
        )
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(6)
    );
}

#[test]
fn apa_native_extractor_maps_direct_player_sources() {
    let extractor = ApaExtractor::new(ExtractorDescriptor::new(
            "APAIE",
            "APA",
            r"(?P<base_url>https?://[^/]+\.apa\.at)/embed/(?P<id>[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12})",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"title: "Native APA title",
                description: "Native APA description",
                poster: "https://cdn.example/apa.jpg",
                hls: "https://cdn.example/apa.m3u8",
                progressive: "https://cdn.example/720.mp4""#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "http://uvp.apa.at/embed/293f6d17-692a-44e3-9fd5-7b178f3a1029",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("293f6d17-692a-44e3-9fd5-7b178f3a1029")
    );
    assert_eq!(result.get_str("title"), Some("Native APA title"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/apa.jpg")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

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

#[test]
fn bitchute_native_extractor_reads_media_and_metadata_apis() {
    let extractor = BitChuteExtractor::new(ExtractorDescriptor::new(
            "BitChuteIE",
            "BitChute",
            r"https?://(?:(?:www|old)\.)?bitchute\.com/(?:video|embed|torrent/[^/?#]+)/(?P<id>[^/?#&]+)",
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "video/media".to_owned(),
                br#"{"media_url":"https://cdn.example/video.mp4"}"#.to_vec(),
            ),
            (
                "api/beta/video".to_owned(),
                br#"{
                        "video_name":"Native BitChute",
                        "description":"Description",
                        "thumbnail_url":"https://cdn.example/thumb.jpg",
                        "view_count":7,
                        "duration":"00:00:16",
                        "hashtags":["bitchute"],
                        "profile_id":"profile1",
                        "channel":{"channel_id":"channel1","channel_name":"Channel"}
                    }"#
                .to_vec(),
            ),
            (
                "api/beta/channel".to_owned(),
                br#"{
                        "profile_name":"Native uploader",
                        "profile_id":"profile1",
                        "channel_name":"Channel",
                        "url_slug":"channel"
                    }"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.bitchute.com/video/abc123/", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("abc123"));
    assert_eq!(result.get_str("title"), Some("Native BitChute"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(16.0)));
    assert_eq!(result.get_str("uploader"), Some("Native uploader"));
    assert_eq!(
        result.get_str("channel_url"),
        Some("https://www.bitchute.com/channel/channel/")
    );
}

#[test]
fn archive_org_native_extractor_maps_metadata_files_and_entry_selection() {
    let extractor = ArchiveOrgExtractor::new(ExtractorDescriptor::new(
        "ArchiveOrgIE",
        "archive.org",
        r"https?://(?:www\.)?archive\.org/(?:details|embed)/(?P<id>[^?#]+)(?:[?].*)?$",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
                "metadata": {
                    "identifier":"demo-item",
                    "title":"Demo archive",
                    "description":"A native archive",
                    "creator":"Archive author",
                    "uploader":"uploader@example.test",
                    "licenseurl":"https://creativecommons.org/publicdomain/zero/1.0/"
                },
                "files": [{
                    "name":"sample video.mp4",
                    "title":"Sample video",
                    "format":"MPEG4",
                    "size":"42",
                    "length":"00:01:02.5",
                    "source":"original"
                }]
            }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://archive.org/details/demo-item/sample%20video.mp4",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("demo-item/sample video.mp4"));
    assert_eq!(result.get_str("title"), Some("Sample video"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(62.5)));
    assert_eq!(
        result.get_str("url"),
        Some("https://archive.org/download/demo-item/sample%20video.mp4")
    );
    assert_eq!(result.get_str("uploader"), Some("uploader@example.test"));
}

#[test]
fn google_drive_native_extractor_maps_playback_transcodes() {
    let extractor = GoogleDriveExtractor::new(ExtractorDescriptor::new(
            "GoogleDriveIE",
            "GoogleDrive",
            r#"(?x)https?://(?:docs|drive|drive\.usercontent)\.google\.com/(?:file/d/|(?:uc|open|download)\?.*?id=)(?P<id>[a-zA-Z0-9_-]{28,})"#,
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
                "mediaMetadata":{"title":"drive video.mp4","duration":9.5},
                "mediaStreamingData":{"formatStreamingData":{
                    "adaptiveTranscodes":[{
                        "url":"https://cdn.example/drive.mp4",
                        "itag":18,
                        "transcodeMetadata":{
                            "mimeType":"video/mp4",
                            "width":640,
                            "height":360,
                            "videoFps":30,
                            "contentLength":"42",
                            "videoCodecString":"h264",
                            "audioCodecString":"aac"
                        }
                    }]
                }},
                "thumbnails":[{"url":"https://cdn.example/thumb.jpg","width":640,"height":360}]
            }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://drive.google.com/file/d/0ByeS4oOUV-49Zzh4R1J6R09zazQ/view",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("0ByeS4oOUV-49Zzh4R1J6R09zazQ"));
    assert_eq!(result.get_str("title"), Some("drive video.mp4"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(9.5)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn bandcamp_native_extractor_reads_track_json_attributes() {
    let extractor = BandcampTrackExtractor::new(ExtractorDescriptor::new(
        "BandcampIE",
        "Bandcamp",
        r"https?://(?P<uploader>[^/]+)\.bandcamp\.com/track/(?P<id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<html><head>
                <meta property="og:image" content="https://cdn.example/art.jpg">
            </head><body>
                <div data-tralbum='{"id":12345,"artist":"Native Artist","current":{"artist":"Native Artist"},"trackinfo":[{"id":12345,"title":"Native Track","duration":42.5,"track_num":2,"file":{"mp3-128":"//cdn.example/track.mp3","flac-999":"https://cdn.example/track.flac"}}]}' data-embed='{"artist":"Native Artist","album_title":"Native Album"}'></div>
                <a class="tag">ambient</a>
            </body></html>"#
                .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://artist.bandcamp.com/track/native-track", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("12345"));
    assert_eq!(
        result.get_str("title"),
        Some("Native Artist - Native Track")
    );
    assert_eq!(result.get_str("album"), Some("Native Album"));
    assert_eq!(result.get_str("ext"), Some("flac"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(42.5)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn banned_video_native_extractor_reads_graphql_metadata_and_comments() {
    let extractor = BannedVideoExtractor::new(ExtractorDescriptor::new(
        "BannedVideoIE",
        "BannedVideo",
        r"https?://(?:www\.)?banned\.video/watch\?id=(?P<id>[0-f]{24})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
                "data":{
                    "getVideo":{
                        "directUrl":"https://cdn.example/video.mp4",
                        "streamUrl":"https://cdn.example/master.m3u8",
                        "live":false,
                        "title":"Native title.",
                        "summary":"Summary",
                        "playCount":12,
                        "largeImage":"https://cdn.example/thumb.jpg",
                        "videoDuration":30.5,
                        "channel":{"_id":"channel1","title":"Channel"},
                        "tags":[{"name":"news"}]
                    },
                    "getVideoComments":[{
                        "_id":"comment1",
                        "content":"Hello",
                        "user":{"_id":"user1","username":"commenter"},
                        "voteCount":{"positive":3},
                        "replyCount":0
                    }]
                }
            }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://banned.video/watch?id=5e7a859644e02200c6ef5f11",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("5e7a859644e02200c6ef5f11"));
    assert_eq!(result.get_str("title"), Some("Native title"));
    assert_eq!(result.get_str("channel"), Some("Channel"));
    assert_eq!(result.get("view_count"), Some(&serde_json::json!(12)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("comments")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn coub_native_extractor_maps_api_versions_and_counters() {
    let extractor = CoubExtractor::new(ExtractorDescriptor::new(
            "CoubIE",
            "Coub",
            r#"(?:coub:|https?://(?:coub\.com/(?:view|embed|coubs)/|c-cdn\.coub\.com/fb-player\.swf\?.*\bcoub(?:ID|id)=))(?P<id>[\da-z]+)"#,
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
                "title":"Native Coub",
                "picture":"https://cdn.example/coub.jpg",
                "duration":4.6,
                "published_at":"2015-04-08T00:00:00Z",
                "views_count":10,
                "likes_count":4,
                "recoubs_count":2,
                "age_restricted":false,
                "channel":{"title":"Native uploader","permalink":"native.uploader"},
                "file_versions":{
                    "html5":{
                        "video":{"low":{"url":"https://cdn.example/low.mp4","size":100}},
                        "audio":{"high":{"url":"https://cdn.example/high.mp3","size":20}}
                    },
                    "iphone":{"url":"https://cdn.example/iphone.mp4"},
                    "mobile":{"audio_url":"https://cdn.example/mobile.mp3"}
                }
            }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("http://coub.com/view/5u5n1", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("5u5n1"));
    assert_eq!(result.get_str("title"), Some("Native Coub"));
    assert_eq!(
        result.get("timestamp"),
        Some(&serde_json::json!(1_428_451_200))
    );
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(0)));
    assert_eq!(result.get_str("uploader_id"), Some("native.uploader"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(4)
    );
}

#[test]
fn vocaroo_native_extractor_builds_head_checked_audio_url() {
    let extractor = VocarooExtractor::new(ExtractorDescriptor::new(
        "VocarooIE",
        "Vocaroo",
        r"https?://(?:www\.)?(?:vocaroo\.com|voca\.ro)/(?:embed/)?(?P<id>\w+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler { body: Vec::new() });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://vocaroo.com/1de8yA3LNe77", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1de8yA3LNe77"));
    assert_eq!(
        result.get_str("url"),
        Some("https://media1.vocaroo.com/mp3/1de8yA3LNe77")
    );
    assert_eq!(result.get_str("ext"), Some("mp3"));
    assert_eq!(result.get_str("vcodec"), Some("none"));
}

#[test]
fn freesound_native_extractor_maps_html_audio_metadata() {
    let extractor = FreesoundExtractor::new(ExtractorDescriptor::new(
        "FreesoundIE",
        "Freesound",
        r"https?://(?:www\.)?freesound\.org/people/[^/]+/sounds/(?P<id>[^/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"<html><head>
                <meta property="og:audio" content="https://freesound.orghttps://cdn.example/sound-lq.mp3">
                <meta property="og:audio:title" content="Native sound">
                <meta property="og:audio:artist" content="Native artist">
            </head><body>
                <div id="sound_description"><p>Description</p></div>
                <span class="duration">12500</span>
                <div class="tags"><a href="/tag/one">one</a><a href="/tag/two">two</a></div>
            </body></html>"#
                .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.freesound.org/people/native/sounds/12345/",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("12345"));
    assert_eq!(result.get_str("title"), Some("Native sound"));
    assert_eq!(result.get_str("uploader"), Some("Native artist"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(12.5)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn yandex_disk_native_extractor_reads_store_and_public_media() {
    let extractor = YandexDiskExtractor::new(ExtractorDescriptor::new(
            "YandexDiskIE",
            "YandexDisk",
            r#"(?x)https?://(?P<domain>yadi\.sk|disk\.(?:360\.)?yandex\.(?:ru|com))/(?:[di]/|public.*?\bhash=)(?P<id>[^/?#&]+)"#,
            true,
        ))
        .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "cloud-api.yandex.net".to_owned(),
                br#"{"href":"https://cdn.example/source.mp4"}"#.to_vec(),
            ),
            (
                "yadi.sk".to_owned(),
                br#"<script id="store-prefetch">{
                        "rootResourceId":"r1",
                        "resources":{"r1":{
                            "name":"native.mp4",
                            "uid":"u1",
                            "meta":{"ext":"mp4","size":"42","views_counter":"7"},
                            "videoStreams":{
                                "duration":12500,
                                "videos":[{
                                    "url":"https://cdn.example/stream.m3u8",
                                    "dimension":"720p",
                                    "size":{"width":1280,"height":720}
                                }]
                            }
                        }},
                        "users":{"u1":{"displayName":"Native user"}}
                    }</script>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://yadi.sk/i/VdOeDou8eZs6Y", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("VdOeDou8eZs6Y"));
    assert_eq!(result.get_str("title"), Some("native.mp4"));
    assert_eq!(result.get_str("uploader"), Some("Native user"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(12.5)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn rumble_embed_native_extractor_maps_formats_live_state_and_captions() {
    let extractor = RumbleEmbedExtractor::new(ExtractorDescriptor::new(
        "RumbleEmbedIE",
        "Rumble",
        r"https?://(?:www\.)?rumble\.com/embed/(?:[0-9a-z]+\.)?(?P<id>[0-9a-z]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
            body: br#"{
                "live":0,
                "duration":234,
                "pubDate":"2019-10-20T00:00:00Z",
                "author":{"name":"Native channel","url":"https://rumble.com/c/native"},
                "i":"https://cdn.example/thumb.jpg",
                "ua":{
                    "mp4":{"720":{"url":"https://cdn.example/video.mp4","meta":{"h":720,"w":1280,"size":42}}},
                    "audio":[{"url":"https://cdn.example/audio.mp3","meta":{"bitrate":128}}],
                    "hls":[{"url":"https://cdn.example/master.m3u8","meta":{"live":false}}]
                },
                "cc":{"en":{"path":"https://cdn.example/en.vtt","language":"English"}}
            }"#
            .to_vec(),
        });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://rumble.com/embed/v5pv5f", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("v5pv5f"));
    assert_eq!(result.get_str("title"), None);
    assert_eq!(result.get_str("channel"), Some("Native channel"));
    assert_eq!(result.get_str("live_status"), Some("not_live"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(234)));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert!(result.get("subtitles").is_some());
}

#[test]
fn clyp_native_extractor_maps_api_formats_in_rust() {
    let extractor = ClypExtractor::new(ExtractorDescriptor::new(
        "ClypIE",
        "Clyp",
        r"https?://(?:www\.)?clyp\.it/(?P<id>[a-z0-9]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br##"{
                "Title": "research",
                "Description": "#Research",
                "Duration": 51.278,
                "OggUrl": "https://cdn.example/research.ogg",
                "Mp3Url": "https://cdn.example/research.mp3"
            }"##
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://clyp.it/iynkjk4b", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("iynkjk4b"));
    assert_eq!(result.get_str("title"), Some("research"));
    assert_eq!(result.get_str("ext"), Some("ogg"));
    assert_eq!(
        result
            .get("formats")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(2)
    );
}

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
