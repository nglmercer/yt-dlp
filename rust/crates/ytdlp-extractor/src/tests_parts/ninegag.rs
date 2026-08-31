#[test]
fn ninegag_native_extractor_maps_animated_api_media() {
    let extractor = NineGagExtractor::new(ExtractorDescriptor::new(
        "NineGagIE",
        "9gag",
        r"https?://(?:www\.)?9gag\.com/gag/(?P<id>[^/?&#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "9gag.com/v1/post?id=abc123".to_owned(),
            br#"{
                "data": {"post": {
                    "type": "Animated",
                    "title": "A &amp; B",
                    "creationTs": 1573237208,
                    "images": {
                        "image1": {
                            "url": "https://cdn.example/post_460.jpg",
                            "width": 460,
                            "height": 460,
                            "webpUrl": "https://cdn.example/post_460.webp"
                        },
                        "image2": {
                            "url": "https://cdn.example/post_360.webm",
                            "width": 360,
                            "height": 360,
                            "duration": 44,
                            "hasAudio": 0,
                            "vp8Url": "https://cdn.example/post_vp8.webm",
                            "vp9Url": "https://cdn.example/post_vp9.webm"
                        }
                    },
                    "creator": {
                        "fullName": "Native Creator",
                        "username": "native_creator",
                        "profileUrl": "https://9gag.com/u/native_creator"
                    },
                    "upVoteCount": 21,
                    "downVoteCount": 3,
                    "commentsCount": 8,
                    "nsfw": 1,
                    "postSection": {"name": "Awesome"},
                    "tags": [{"key": "Awesome"}, {"key": "rust"}]
                }}
            }"#
            .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://9gag.com/gag/abc123", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("abc123"));
    assert_eq!(result.get_str("title"), Some("A & B"));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1573237208)));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(44)));
    assert_eq!(result.get_str("uploader"), Some("Native Creator"));
    assert_eq!(result.get_str("uploader_id"), Some("native_creator"));
    assert_eq!(
        result.get_str("uploader_url"),
        Some("https://9gag.com/u/native_creator")
    );
    assert_eq!(result.get("like_count"), Some(&serde_json::json!(21)));
    assert_eq!(result.get("dislike_count"), Some(&serde_json::json!(3)));
    assert_eq!(result.get("comment_count"), Some(&serde_json::json!(8)));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(18)));
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["Awesome"]))
    );
    assert_eq!(
        result.get("tags"),
        Some(&serde_json::json!(["Awesome", "rust"]))
    );

    let thumbnails = result
        .get("thumbnails")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(thumbnails.len(), 2);
    assert!(thumbnails.iter().any(|thumbnail| {
        thumbnail.get("id") == Some(&serde_json::json!("1-webp"))
            && thumbnail.get("url") == Some(&serde_json::json!("https://cdn.example/post_460.webp"))
    }));

    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 3);
    assert!(formats.iter().any(|format| {
        format.get("format_id") == Some(&serde_json::json!("2-vp8"))
            && format.get("vcodec") == Some(&serde_json::json!("vp8"))
            && format.get("acodec") == Some(&serde_json::json!("none"))
    }));
    assert!(formats.iter().any(|format| {
        format.get("format_id") == Some(&serde_json::json!("2"))
            && format.get("ext") == Some(&serde_json::json!("webm"))
    }));
}

#[test]
fn ninegag_native_extractor_marks_static_posts_as_todo() {
    let extractor = NineGagExtractor::new(ExtractorDescriptor::new(
        "NineGagIE",
        "9gag",
        r"https?://(?:www\.)?9gag\.com/gag/(?P<id>[^/?&#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "9gag.com/v1/post?id=static1".to_owned(),
            br#"{"data":{"post":{"type":"Photo"}}}"#.to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://9gag.com/gag/static1", &context)
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
