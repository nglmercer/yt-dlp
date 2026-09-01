struct LoomHandler;

impl RequestHandler for LoomHandler {
    fn name(&self) -> &str {
        "loom-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if request
            .url()
            .contains("/api/campaigns/sessions/43d05f362f734614a2e81b4694a3a523/raw-url")
        {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{"url":"https://cdn.example/loom/raw.mp4?token=raw"}"#.to_vec(),
            ));
        }
        if request.url().contains(
            "/api/campaigns/sessions/43d05f362f734614a2e81b4694a3a523/transcoded-url",
        ) {
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                br#"{"url":"https://cdn.example/loom/transcoded.webm"}"#.to_vec(),
            ));
        }
        if request.url().contains("www.loom.com/graphql") {
            let operation = request
                .data()
                .and_then(|data| serde_json::from_slice::<serde_json::Value>(data).ok())
                .and_then(|data| {
                    data.get("operationName")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_default();
            let body = match operation.as_str() {
                "GetVideoSSR" => serde_json::json!({
                    "data": {
                        "getVideo": {
                            "__typename": "RegularUserVideo",
                            "id": "43d05f362f734614a2e81b4694a3a523",
                            "createdAt": "2024-01-02T03:04:05Z",
                            "description": "Native Loom description",
                            "name": "Native Loom video",
                            "owner": {"display_name": "Native Loom owner"},
                            "video_properties": {
                                "duration": 93,
                                "width": 1280,
                                "height": 720,
                                "microphone_enabled": false
                            }
                        }
                    }
                }),
                "GetVideoSource" => serde_json::json!({
                    "data": {
                        "getVideo": {
                            "__typename": "RegularUserVideo",
                            "nullableRawCdnUrl": {
                                "url": "https://cdn.example/loom/master.m3u8?token=cdn"
                            }
                        }
                    }
                }),
                "FetchVideoTranscript" => serde_json::json!({
                    "data": {
                        "fetchVideoTranscript": {
                            "__typename": "VideoTranscriptDetails",
                            "source_url": "https://cdn.example/loom/captions.vtt"
                        }
                    }
                }),
                "FetchChapters" => serde_json::json!({
                    "data": {
                        "fetchVideoChapters": {
                            "__typename": "VideoChapters",
                            "content": "00:00 Intro\n00:10 Main section"
                        }
                    }
                }),
                _ => serde_json::json!({"errors": [{"message": "unknown operation"}]}),
            };
            return Ok(Response::new(
                request.url(),
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Loom route for {}", request.url()),
        ))
    }
}

fn loom_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LoomHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn loom_native_extractor_maps_graphql_sources_subtitles_and_chapters() {
    let extractor = LoomExtractor::new(ExtractorDescriptor::new(
        "LoomIE",
        "loom",
        r"https?://(?:www\.)?loom\.com/(?:share|embed)/(?P<id>[\da-f]{32})",
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.loom.com/share/43d05f362f734614a2e81b4694a3a523",
            &loom_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result.get_str("id"),
        Some("43d05f362f734614a2e81b4694a3a523")
    );
    assert_eq!(result.get_str("title"), Some("Native Loom video"));
    assert_eq!(result.get_str("uploader"), Some("Native Loom owner"));
    assert_eq!(result.get_i64("duration"), Some(93));
    assert_eq!(result.get_i64("timestamp"), Some(1_704_164_645));
    assert_eq!(result.get_str("acodec"), Some("none"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("http-raw")));
    assert_eq!(formats[0].get("acodec"), Some(&serde_json::json!("none")));
    assert_eq!(
        formats[2].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("en"))
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .and_then(|track| track.get("url")),
        Some(&serde_json::json!("https://cdn.example/loom/captions.vtt"))
    );
    assert_eq!(
        result
            .get("chapters")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        result
            .get("chapters")
            .and_then(serde_json::Value::as_array)
            .and_then(|chapters| chapters.last())
            .and_then(|chapter| chapter.get("end_time")),
        Some(&serde_json::json!(93))
    );
}

#[test]
fn loom_native_extractor_marks_password_protection_as_todo() {
    struct PasswordHandler;

    impl RequestHandler for PasswordHandler {
        fn name(&self) -> &str {
            "loom-password-test"
        }

        fn supports(&self, _request: &Request) -> Result<(), RequestError> {
            Ok(())
        }

        fn send(&self, request: &Request) -> Result<Response, RequestError> {
            let body = serde_json::json!({
                "data": {
                    "getVideo": {
                        "__typename": "VideoPasswordMissingOrIncorrect",
                        "id": "43d05f362f734614a2e81b4694a3a523"
                    }
                }
            });
            Ok(Response::new(
                request.url(),
                200,
                "OK",
                serde_json::to_vec(&body).unwrap(),
            ))
        }
    }

    let mut director = RequestDirector::new();
    director.add_handler(PasswordHandler);
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let extractor = LoomExtractor::new(ExtractorDescriptor::new(
        "LoomIE",
        "loom",
        r"https?://(?:www\.)?loom\.com/(?:share|embed)/(?P<id>[\da-f]{32})",
        true,
    ))
    .unwrap();
    let error = extractor
        .extract_with_context(
            "https://www.loom.com/share/43d05f362f734614a2e81b4694a3a523",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
