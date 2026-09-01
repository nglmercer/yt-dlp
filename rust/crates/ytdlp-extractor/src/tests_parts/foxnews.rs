#[test]
fn foxnews_native_extractor_maps_jsonp_amp_feed() {
    let extractor = FoxNewsExtractor::new(ExtractorDescriptor::new(
        "FoxNewsIE",
        "foxnews",
        r"https?://video\.(?:insider\.)?fox(?:news|business)\.com/v/(?:video-embed\.html\?video_id=)?(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"uid_6320653836112({"channel":{"item":{
            "guid":"feed-guid",
            "pubDate":"2023-02-17T12:22:24Z",
            "media-group":{
                "media-title":"Native Fox title",
                "media-description":"Native Fox description",
                "media-thumbnail":[{"@attributes":{"url":"https://cdn.example/poster.jpg","width":"1280","height":"720"}}],
                "media-subTitle":{"@attributes":{"href":"https://cdn.example/captions.vtt","lang":"en","type":"text/vtt"}},
                "media-content":[
                    {"@attributes":{"url":"https://cdn.example/master.m3u8","type":"application/x-mpegURL","duration":"404"}},
                    {"@attributes":{"url":"https://cdn.example/high.mp4","type":"video/mp4","bitrate":"800","fileSize":"10"},"media-category":{"@attributes":{"label":"high"}}}
                ]
            }
        }}})"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://video.foxnews.com/v/6320653836112",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("6320653836112"));
    assert_eq!(result.get_str("title"), Some("Native Fox title"));
    assert_eq!(
        result.get_str("description"),
        Some("Native Fox description")
    );
    assert_eq!(result.get_i64("duration"), Some(404));
    assert!(result.get_i64("timestamp").is_some());
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/poster.jpg")
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
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("ext")),
        Some(&serde_json::json!("vtt"))
    );
}

#[test]
fn foxnews_native_extractor_marks_f4m_as_todo() {
    let extractor = FoxNewsExtractor::new(ExtractorDescriptor::new(
        "FoxNewsIE",
        "foxnews",
        r"https?://video\.(?:insider\.)?fox(?:news|business)\.com/v/(?:video-embed\.html\?video_id=)?(?P<id>\d+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"channel":{"item":{"guid":"1","media-content":{"@attributes":{"url":"https://cdn.example/manifest.f4m","type":"application/f4m"}}}}}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://video.foxnews.com/v/1", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
