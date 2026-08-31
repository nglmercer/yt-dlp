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
