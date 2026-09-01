#[test]
fn filmon_native_extractor_maps_vod_streams_and_thumbnails() {
    let extractor = FilmOnExtractor::new(ExtractorDescriptor::new(
        "FilmOnIE",
        "filmon",
        r#"(?:https?://(?:www\.)?filmon\.com/vod/view/|filmon:)(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
            "response":{
                "title":"Native FilmOn movie",
                "description":"Native description",
                "streams":{
                    "low":{"url":"https://cdn.example/low.m3u8","quality":"low"},
                    "high":{"url":"https://cdn.example/high.m3u8","quality":"high"}
                },
                "poster":{
                    "url":"https://cdn.example/poster.jpg",
                    "width":1200,
                    "height":680,
                    "thumbs":{
                        "small":{"url":"https://cdn.example/small.jpg","width":160,"height":90}
                    }
                }
            }
        }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.filmon.com/vod/view/24869-0-native-movie",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("24869"));
    assert_eq!(result.get_str("title"), Some("Native FilmOn movie"));
    assert_eq!(result.get_str("description"), Some("Native description"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("m3u8_native")));
    assert!(formats
        .iter()
        .any(|format| format.get("quality") == Some(&serde_json::json!(1))));
    let thumbnails = result
        .get("thumbnails")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(thumbnails.len(), 2);
    assert_eq!(thumbnails[1].get("id"), Some(&serde_json::json!("poster")));
}

#[test]
fn filmon_native_extractor_maps_series_playlist_entries() {
    let extractor = FilmOnExtractor::new(ExtractorDescriptor::new(
        "FilmOnIE",
        "filmon",
        r#"(?:https?://(?:www\.)?filmon\.com/vod/view/|filmon:)(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"response":{"title":"Native series","description":"Series description","type_id":1,"episodes":[101,"102"]}}"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("filmon:999", &context)
        .unwrap()
    else {
        panic!("expected FilmOn playlist");
    };

    assert_eq!(info.get_str("id"), Some("999"));
    assert_eq!(info.get_str("title"), Some("Native series"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("_type"), Some("url"));
    assert_eq!(entries[0].get_str("url"), Some("filmon:101"));
    assert_eq!(entries[1].get_str("url"), Some("filmon:102"));
}

#[test]
fn filmon_channel_native_extractor_maps_live_state_and_streams() {
    let extractor = FilmOnChannelExtractor::new(ExtractorDescriptor::new(
        "FilmOnChannelIE",
        "filmon:channel",
        r#"https?://(?:www\.)?filmon\.com/(?:tv|channel)/(?P<id>[a-z0-9-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"data":{
            "id":4190,
            "alias":"sports-haters",
            "title":"Native channel",
            "description":"Channel description",
            "is_vod":true,
            "is_vox":false,
            "streams":[{"quality":"high","url":"https://cdn.example/channel.m3u8"}]
        }}"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.filmon.com/tv/sports-haters",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("4190"));
    assert_eq!(result.get_str("display_id"), Some("sports-haters"));
    assert_eq!(result.get_str("title"), Some("Native channel"));
    assert_eq!(result.get_bool("is_live"), Some(false));
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
fn filmon_channel_native_extractor_marks_rtmp_only_streams_as_todo() {
    let extractor = FilmOnChannelExtractor::new(ExtractorDescriptor::new(
        "FilmOnChannelIE",
        "filmon:channel",
        r#"https?://(?:www\.)?filmon\.com/(?:tv|channel)/(?P<id>[a-z0-9-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"data":{"id":4190,"title":"RTMP channel","streams":[{"url":"rtmp://stream.example/live"}]}}"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.filmon.com/channel/rtmp", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
