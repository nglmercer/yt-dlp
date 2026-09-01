#[test]
fn egghead_lesson_native_extractor_maps_media_and_metadata() {
    let extractor = EggheadLessonExtractor::new(ExtractorDescriptor::new(
        "EggheadLessonIE",
        "egghead:lesson",
        r"https?://(?:app\.)?egghead\.io/(?:api/v1/)?lessons/(?P<id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{
            "id":1196,
            "title":"Native Egghead lesson",
            "summary":"A lesson summary",
            "thumb_nail":"https://cdn.example/lesson.jpg",
            "published_at":"2016-12-09T12:00:00Z",
            "duration":304,
            "plays_count":7,
            "tag_list":["rust","native"],
            "series":{"title":"Native series"},
            "media_urls":{
                "hls":"https://cdn.example/lesson.m3u8",
                "dash":"https://cdn.example/lesson.mpd",
                "mp4":"https://cdn.example/lesson.mp4"
            }
        }"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://app.egghead.io/lessons/native-rust-lesson",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1196"));
    assert_eq!(result.get_str("display_id"), Some("native-rust-lesson"));
    assert_eq!(result.get_str("title"), Some("Native Egghead lesson"));
    assert_eq!(result.get_i64("duration"), Some(304));
    assert_eq!(result.get_i64("view_count"), Some(7));
    assert!(result.get_i64("timestamp").is_some());
    assert_eq!(result.get_str("series"), Some("Native series"));
    assert_eq!(
        result
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
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
            .and_then(|formats| {
                formats
                    .iter()
                    .find(|format| format.get("format_id") == Some(&serde_json::json!("hls")))
            })
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn egghead_course_native_extractor_builds_lesson_playlist() {
    let extractor = EggheadCourseExtractor::new(ExtractorDescriptor::new(
        "EggheadCourseIE",
        "egghead:course",
        r"https?://(?:app\.)?egghead\.io/(?:course|playlist)s/(?P<id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "series/native-course/lessons".to_owned(),
                br#"[{"id":101,"http_url":"https://egghead.io/lessons/native-one"},{"id":"202","http_url":"https://egghead.io/lessons/native-two"},{"id":303}]"#.to_vec(),
            ),
            (
                "series/native-course".to_owned(),
                br#"{"id":432655,"title":"Native Egghead course","description":"Course description"}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://egghead.io/courses/native-course",
            &context,
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("Egghead course must return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("432655"));
    assert_eq!(info.get_str("title"), Some("Native Egghead course"));
    assert_eq!(info.get_str("description"), Some("Course description"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("url"), Some("https://egghead.io/lessons/native-one"));
    assert_eq!(entries[0].get_str("ie_key"), Some("egghead:lesson"));
    assert_eq!(entries[0].get_str("id"), Some("101"));
    assert_eq!(entries[1].get_str("id"), Some("202"));
}

#[test]
fn egghead_lesson_native_extractor_requires_media_urls() {
    let extractor = EggheadLessonExtractor::new(ExtractorDescriptor::new(
        "EggheadLessonIE",
        "egghead:lesson",
        r"https?://(?:app\.)?egghead\.io/(?:api/v1/)?lessons/(?P<id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{"id":1196,"title":"No media lesson","media_urls":{}}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://egghead.io/lessons/no-media",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Extraction);
    assert!(error.message.contains("no playable media URLs"));
}
