#[test]
fn frontendmasters_native_extractor_maps_sources_and_transcript() {
    let extractor = FrontendMastersExtractor::new(ExtractorDescriptor::new(
        "FrontendMastersIE",
        "FrontendMasters",
        r#"(?:frontendmasters:|https?://api\.frontendmasters\.com/v\d+/kabuki/video/)(?P<id>[^/]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "video/native-lesson/source?f=webm&r=360".to_owned(),
                br#"{"url":"https://cdn.example/native-lesson.webm"}"#.to_vec(),
            ),
            (
                "video/native-lesson/source?f=mp4&r=1080".to_owned(),
                br#"{"url":"https://cdn.example/native-lesson.mp4"}"#.to_vec(),
            ),
            (
                "video/native-lesson/source".to_owned(),
                br#"{}"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("frontendmasters:native-lesson", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-lesson"));
    assert_eq!(result.get_str("title"), Some("native-lesson"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("width"), Some(&serde_json::json!(480)));
    assert_eq!(formats[1].get("height"), Some(&serde_json::json!(1080)));
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!(
            "https://api.frontendmasters.com/v1/kabuki/transcripts/native-lesson.vtt"
        ))
    );
}

#[test]
fn frontendmasters_native_extractor_marks_account_gated_sources_as_todo() {
    let extractor = FrontendMastersExtractor::new(ExtractorDescriptor::new(
        "FrontendMastersIE",
        "FrontendMasters",
        r#"(?:frontendmasters:|https?://api\.frontendmasters\.com/v\d+/kabuki/video/)(?P<id>[^/]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"{}"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("frontendmasters:private-lesson", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}

fn frontendmasters_course_fixture() -> Vec<u8> {
    br#"{
        "title":"Native course",
        "description":"Native course description",
        "lessonElements":["https://frontendmasters.com/chapters/introduction"],
        "lessonData":{
            "second":{
                "slug":"second",
                "hash":"lesson-second",
                "title":"Second lesson",
                "index":2,
                "elementIndex":3,
                "timestamp":"00:10:00 - 00:15:30"
            },
            "first":{
                "slug":"first",
                "statsId":"lesson-first",
                "title":"First lesson",
                "description":"First description",
                "thumbnail":"https://cdn.example/first.jpg",
                "index":0,
                "elementIndex":1,
                "timestamp":"00:00:00 - 00:07:30"
            }
        }
    }"#
    .to_vec()
}

#[test]
fn frontendmasters_lesson_native_extractor_maps_course_lesson() {
    let extractor = FrontendMastersLessonExtractor::new(ExtractorDescriptor::new(
        "FrontendMastersLessonIE",
        "FrontendMastersLesson",
        r#"https?://(?:www\.)?frontendmasters\.com/courses/(?P<course_name>[^/]+)/(?P<lesson_name>[^/]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: frontendmasters_course_fixture(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://frontendmasters.com/courses/native-course/first",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(result.get_str("url"), Some("frontendmasters:lesson-first"));
    assert_eq!(result.get_str("ie_key"), Some("FrontendMasters"));
    assert_eq!(result.get_str("title"), Some("First lesson"));
    assert_eq!(result.get_f64("duration"), Some(450.0));
    assert_eq!(result.get_i64("chapter_number"), Some(1));
    assert_eq!(
        result.get_str("chapter"),
        Some("https://frontendmasters.com/chapters/introduction")
    );
}

#[test]
fn frontendmasters_course_native_extractor_builds_sorted_playlist() {
    let extractor = FrontendMastersCourseExtractor::new(ExtractorDescriptor::new(
        "FrontendMastersCourseIE",
        "FrontendMastersCourse",
        r#"https?://(?:www\.)?frontendmasters\.com/courses/(?P<id>[^/]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: frontendmasters_course_fixture(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://frontendmasters.com/courses/native-course/",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Frontend Masters playlist");
    };

    assert_eq!(info.get_str("id"), Some("native-course"));
    assert_eq!(info.get_str("title"), Some("Native course"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("url"), Some("frontendmasters:lesson-first"));
    assert_eq!(
        entries[1].get_str("url"),
        Some("frontendmasters:lesson-second")
    );
}
