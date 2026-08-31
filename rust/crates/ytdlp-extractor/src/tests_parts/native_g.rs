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
