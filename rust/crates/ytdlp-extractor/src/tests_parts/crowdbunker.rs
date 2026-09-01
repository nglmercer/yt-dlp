#[test]
fn crowdbunker_native_extractor_maps_api_manifests_captions_and_metadata() {
    let extractor = CrowdBunkerExtractor::new(ExtractorDescriptor::new(
        "CrowdBunkerIE",
        "CrowdBunker",
        r"https?://(?:www\.)?crowdbunker\.com/v/(?P<id>[^/?#$&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "api.divulg.org/post/native-video/details".to_owned(),
            br#"{
                "video":{
                    "title":"Native CrowdBunker",
                    "description":"Native video description",
                    "viewCount":12345,
                    "duration":5386,
                    "publishedAt":"2021-12-18T00:00:00Z",
                    "dashManifest":{"url":"https://cdn.example/crowd/native.mpd"},
                    "hlsManifest":{"url":"https://cdn.example/crowd/native.m3u8"},
                    "captions":[{"languageCode":"fr","file":{"url":"https://cdn.example/crowd/fr.vtt"}}],
                    "thumbnails":[{"url":"https://cdn.example/crowd/poster.jpg","width":1280,"height":720}]
                },
                "channel":{"name":"Native Channel","id":"native-channel"},
                "likesCount":321
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://crowdbunker.com/v/native-video", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-video"));
    assert_eq!(result.get_str("title"), Some("Native CrowdBunker"));
    assert_eq!(result.get_str("uploader"), Some("Native Channel"));
    assert_eq!(result.get_str("uploader_id"), Some("native-channel"));
    assert_eq!(result.get_i64("view_count"), Some(12345));
    assert_eq!(result.get_i64("like_count"), Some(321));
    assert_eq!(result.get_str("upload_date"), Some("20211218"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/crowd/native.mpd"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("protocol"), Some(&serde_json::json!("http_dash_segments")));
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("fr"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!("https://cdn.example/crowd/fr.vtt"))
    );
}

#[test]
fn crowdbunker_channel_native_extractor_follows_cursor_pages() {
    let extractor = CrowdBunkerChannelExtractor::new(ExtractorDescriptor::new(
        "CrowdBunkerChannelIE",
        "CrowdBunkerChannel",
        r"https?://(?:www\.)?crowdbunker\.com/@(?P<id>[^/?#$&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "organization/native-channel/posts?after=cursor-2".to_owned(),
                br#"{"items":[{"uid":"video-two"}]}"#.to_vec(),
            ),
            (
                "organization/native-channel/posts".to_owned(),
                br#"{"items":[{"uid":"video-one"}],"last":"cursor-2"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context("https://crowdbunker.com/@native-channel", &context)
        .unwrap()
    else {
        panic!("expected CrowdBunker channel playlist");
    };

    assert_eq!(info.get_str("id"), Some("native-channel"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("CrowdBunker"));
    assert_eq!(
        entries[1].get_str("url"),
        Some("https://crowdbunker.com/v/video-two")
    );
}
