struct LecturioHandler;

impl RequestHandler for LecturioHandler {
    fn name(&self) -> &str {
        "lecturio-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("/api/en/latest/html5/lecture/native-lecture.json") {
            let body = serde_json::json!({
                "title": "Native Lecturio lecture",
                "productId": "course_39634",
                "content": {
                    "media": [
                        {
                            "file": "https://cdn.example/lecturio-720.mp4",
                            "label": "720p (HD)",
                            "fileSize": 1234
                        },
                        {
                            "file": "https://cdn.example/lecturio.smil",
                            "label": "SMIL",
                            "fileSize": 10
                        }
                    ]
                },
                "captions": [
                    {
                        "url": "https://cdn.example/en_native.vtt",
                        "languageCode": "English",
                        "translatedCode": "English"
                    },
                    {
                        "url": "https://cdn.example/es_en_native.vtt",
                        "languageCode": "Spanish",
                        "translatedCode": "Spanish auto-translated"
                    }
                ]
            });
            return Ok(Response::new(
                url,
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        if url.contains("/api/en/latest/html5/course/content/native-course.json") {
            let body = serde_json::json!({
                "title": "Native Lecturio course",
                "description": "<p>Native course description</p>",
                "lectures": [
                    {
                        "id": 39634,
                        "url": "/medical-courses/native-lecture.lecture"
                    },
                    {
                        "id": 39635,
                        "url": "https://app.lecturio.com/medical-courses/native-second.lecture"
                    }
                ]
            });
            return Ok(Response::new(
                url,
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        if url.contains("lecturio.de/jura/native-course.kurs") {
            let page = r#"
                <h1>Native German course</h1>
                <table>
                    <td data-lecture-id="501"><span><a href="/jura/native-one.vortrag">one</a></span></td>
                    <td data-lecture-id="502"><span><a href="https://www.lecturio.de/jura/native-two.vortrag">two</a></span></td>
                </table>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Lecturio route for {url}"),
        ))
    }
}

fn lecturio_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LecturioHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn lecturio_native_extractor_maps_media_and_captions() {
    let extractor = LecturioExtractor::new(ExtractorDescriptor::new(
        "LecturioIE",
        "Lecturio",
        r#"(?x)https://(?:app\.lecturio\.com/([^/?#]+/(?P<nt>[^/?#&]+)\.lecture|(?:\#/)?lecture/c/\d+/(?P<id>\d+))|(?:www\.)?lecturio\.de/(?:[^/?#]+/)+(?P<nt_de>[^/?#&]+)\.vortrag)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://app.lecturio.com/medical-courses/native-lecture.lecture#tab/videos",
            &lecturio_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("39634"));
    assert_eq!(result.get_str("title"), Some("Native Lecturio lecture"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 1);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("HD")));
    assert_eq!(formats[0].get("height"), Some(&serde_json::json!(720)));
    assert_eq!(formats[0].get("filesize"), Some(&serde_json::json!(1_234_000)));
    assert_eq!(
        result.get("subtitles"),
        Some(&serde_json::json!({
            "en": [{"url": "https://cdn.example/en_native.vtt"}]
        }))
    );
    assert_eq!(
        result.get("automatic_captions"),
        Some(&serde_json::json!({
            "es": [{"url": "https://cdn.example/es_en_native.vtt"}]
        }))
    );
}

#[test]
fn lecturio_course_native_extractor_builds_transparent_entries() {
    let extractor = LecturioCourseExtractor::new(ExtractorDescriptor::new(
        "LecturioCourseIE",
        "LecturioCourse",
        r#"https?://app\.lecturio\.com/(?:[^/]+/(?P<nt>[^/?#&]+)\.course|(?:\#/)?course/c/(?P<id>\d+))"#,
        true,
    ))
    .unwrap();
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://app.lecturio.com/medical-courses/native-course.course#/",
            &lecturio_context(),
        )
        .unwrap()
    else {
        panic!("expected Lecturio course playlist");
    };
    assert_eq!(info.get_str("id"), Some("native-course"));
    assert_eq!(info.get_str("title"), Some("Native Lecturio course"));
    assert_eq!(info.get_str("description"), Some("Native course description"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("Lecturio"));
    assert_eq!(entries[0].get_str("id"), Some("39634"));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://app.lecturio.com/medical-courses/native-lecture.lecture")
    );
}

#[test]
fn lecturio_german_course_native_extractor_maps_lecture_links() {
    let extractor = LecturioDeCourseExtractor::new(ExtractorDescriptor::new(
        "LecturioDeCourseIE",
        "LecturioDeCourse",
        r#"https?://(?:www\.)?lecturio\.de/[^/]+/(?P<id>[^/?#&]+)\.kurs"#,
        true,
    ))
    .unwrap();
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.lecturio.de/jura/native-course.kurs",
            &lecturio_context(),
        )
        .unwrap()
    else {
        panic!("expected Lecturio German course playlist");
    };
    assert_eq!(info.get_str("id"), Some("native-course"));
    assert_eq!(info.get_str("title"), Some("Native German course"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("ie_key"), Some("Lecturio"));
    assert_eq!(entries[0].get_str("id"), Some("501"));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://www.lecturio.de/jura/native-one.vortrag")
    );
}
