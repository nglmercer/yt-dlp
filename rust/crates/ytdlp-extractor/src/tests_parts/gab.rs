#[test]
fn gab_native_extractor_maps_video_status_and_formats() {
    let extractor = GabExtractor::new(ExtractorDescriptor::new(
        "GabIE",
        "Gab",
        r#"https?://(?:www\.)?gab\.com/[^/]+/posts/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"created_at":"2021-10-25T20:04:49Z",
            "content":"<p>Native <strong>Gab</strong> post</p>",
            "favourites_count":7,"replies_count":3,"reblogs_count":2,
            "account":{"display_name":"Native Author","username":"native",
                "id":946600,"url":"https://gab.com/native"},
            "media_attachments":[{
                "type":"video","url":"https://cdn.example/original.webm",
                "source_mp4":"https://cdn.example/playable.mp4",
                "meta":{"duration":12.5,"length":"00:00:12",
                    "original":{"width":1920,"height":1080,"bitrate":4,"fps":30},
                    "playable":{"width":1280,"height":720,"bitrate":2,
                        "audio_encode":"aac"}}
            }]}"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://gab.com/native/posts/107163961867310434",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("107163961867310434-0"));
    assert_eq!(result.get_str("title"), Some("Native Author on Gab"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Gab post")
    );
    assert_eq!(result.get_i64("timestamp"), Some(1_635_192_289));
    assert_eq!(result.get_f64("duration"), Some(12.5));
    assert_eq!(result.get_str("uploader"), Some("native"));
    assert_eq!(result.get_str("uploader_id"), Some("946600"));
    assert_eq!(result.get_i64("like_count"), Some(7));
    assert_eq!(result.get_i64("comment_count"), Some(3));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/original.webm")
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
fn gab_native_extractor_returns_playlist_for_multiple_video_attachments() {
    let extractor = GabExtractor::new(ExtractorDescriptor::new(
        "GabIE",
        "Gab",
        r#"https?://(?:www\.)?gab\.com/[^/]+/posts/(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"account":{"display_name":"Native Author","username":"native"},
            "media_attachments":[
                {"type":"video","url":"https://cdn.example/one.mp4",
                 "meta":{"original":{}}},
                {"type":"gifv","url":"https://cdn.example/two.gif",
                 "source_mp4":"https://cdn.example/two.mp4",
                 "meta":{"original":{}}}
            ]}"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://gab.com/native/posts/123", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("playlist"));
    assert_eq!(
        result
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}
