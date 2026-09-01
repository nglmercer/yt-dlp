#[test]
fn fourtube_native_extractor_maps_metadata_and_token_formats() {
    let extractor = FourTubeExtractor::new(ExtractorDescriptor::new(
        "FourTubeIE",
        "4tube",
        r#"https?://(?:(?P<kind>www|m)\.)?4tube\.com/(?:videos|embed)/(?P<id>\d+)(?:/(?P<display_id>[^/?#&]+))?"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "token.4tube.com/4242/480+720/desktop".to_owned(),
                br#"{
                    "480":{"token":"https://cdn.example/4242-480.mp4"},
                    "720":{"token":"https://cdn.example/4242-720.mp4"}
                }"#
                .to_vec(),
            ),
            (
                "4tube.com/videos/4242/native-video".to_owned(),
                br#"<html><head>
                    <meta name="name" content="Native 4tube title">
                    <meta name="uploadDate" content="2013-10-31T12:38:12Z">
                    <meta name="thumbnailUrl" content="https://cdn.example/4242.jpg">
                    <meta name="duration" content="PT9M43S">
                    <meta itemprop="interactionCount" content="UserPlays:1,234">
                    <meta itemprop="interactionCount" content="UserLikes:56">
                </head><body>
                    <a class="item-to-subscribe" href="/channel/native-channel"
                        title="Go to Native Channel page"></a>
                    <div>Categories / Tags <ul class="tag list">
                        <li><a>Category One</a></li><li><a>Category Two</a></li>
                    </ul></div>
                    <button data-id="4242" data-quality="480"></button>
                    <button data-id="4242" data-quality="720"></button>
                </body></html>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.4tube.com/videos/4242/native-video",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("4242"));
    assert_eq!(result.get_str("title"), Some("Native 4tube title"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/4242.jpg"));
    assert_eq!(result.get_str("uploader"), Some("Native Channel"));
    assert_eq!(result.get_str("uploader_id"), Some("native-channel"));
    assert_eq!(result.get_i64("timestamp"), Some(1383223092));
    assert_eq!(result.get_str("upload_date"), Some("20131031"));
    assert_eq!(result.get_f64("duration"), Some(583.0));
    assert_eq!(result.get_i64("view_count"), Some(1234));
    assert_eq!(result.get_i64("like_count"), Some(56));
    assert_eq!(result.get("age_limit"), Some(&serde_json::json!(18)));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("480p")));
    assert_eq!(formats[0].get("quality"), Some(&serde_json::json!(480)));
    assert_eq!(
        formats[1].get("url"),
        Some(&serde_json::json!("https://cdn.example/4242-720.mp4"))
    );
}

#[test]
fn fourtube_native_extractor_maps_player_bootstrap_parameters() {
    let extractor = FourTubeExtractor::new(ExtractorDescriptor::new(
        "FuxIE",
        "Fux",
        r#"https?://(?:(?P<kind>www|m)\.)?fux\.com/(?:video|embed)/(?P<id>\d+)(?:/(?P<display_id>[^/?#&]+))?"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "token.fux.com/5353/360+720/desktop".to_owned(),
                br#"{"360":{"token":"https://cdn.example/5353-360.mp4"},"720":{"token":"https://cdn.example/5353-720.mp4"}}"#
                    .to_vec(),
            ),
            (
                "player.example/embed.js".to_owned(),
                br#"<script>$.ajax(url, opts); } }) (5353, 0, [360, 720])</script>"#.to_vec(),
            ),
            (
                "fux.com/video/5353/native".to_owned(),
                br#"<meta name="name" content="Bootstrap title">
                    <script id="playerembed" src="https://player.example/embed.js"></script>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.fux.com/video/5353/native", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("5353"));
    assert_eq!(result.get_str("title"), Some("Bootstrap title"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("format_id")),
        Some(&serde_json::json!("360p"))
    );
}

#[test]
fn porntube_native_extractor_maps_initial_state() {
    let extractor = FourTubeExtractor::new(ExtractorDescriptor::new(
        "PornTubeIE",
        "PornTube",
        r#"https?://(?:(?P<kind>www|m)\.)?porntube\.com/(?:videos/(?P<display_id>[^/]+)_|embed/)(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "tkn.porntube.com/media-7089759/480+720/desktop".to_owned(),
                br#"{"480":{"token":"https://cdn.example/7089759-480.mp4"},"720":{"token":"https://cdn.example/7089759-720.mp4"}}"#
                    .to_vec(),
            ),
            (
                "porntube.com/videos/teen_7089759".to_owned(),
                br#"<script>INITIALSTATE="eyJwYWdlIjp7InZpZGVvIjp7ImlkIjoiNzA4OTc1OSIsInRpdGxlIjoiVmlkZW8gdGl0bGUiLCJtZWRpYUlkIjoibWVkaWEtNzA4OTc1OSIsImVuY29kaW5ncyI6W3siaGVpZ2h0Ijo0ODB9LHsiaGVpZ2h0Ijo3MjB9XSwibWFzdGVyVGh1bWIiOiJodHRwczovL2Nkbi5leGFtcGxlL3RodW1iLmpwZyIsInVzZXIiOnsidXNlcm5hbWUiOiJuYXRpdmUtdXNlciIsImlkIjo5OX0sImxpa2VzIjo3LCJkaXNsaWtlcyI6MSwicGxheXNRdHkiOjEyMywiZHVyYXRpb25JblNlY29uZHMiOjEwMSwicHVibGlzaGVkQXQiOiIyMDIyLTA2LTAxVDEyOjAwOjAwWiJ9fX0="</script>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.porntube.com/videos/teen_7089759",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("title"), Some("Video title"));
    assert_eq!(result.get_str("uploader"), Some("native-user"));
    assert_eq!(result.get_str("uploader_id"), Some("99"));
    assert_eq!(result.get_i64("like_count"), Some(7));
    assert_eq!(result.get_i64("dislike_count"), Some(1));
    assert_eq!(result.get_i64("view_count"), Some(123));
    assert_eq!(result.get_f64("duration"), Some(101.0));
    assert_eq!(result.get_str("upload_date"), Some("20220601"));
}

#[test]
fn fourtube_native_extractor_marks_unknown_bootstrap_as_todo() {
    let extractor = FourTubeExtractor::new(ExtractorDescriptor::new(
        "PornerBrosIE",
        "PornerBros",
        r#"https?://(?:(?P<kind>www|m)\.)?pornerbros\.com/(?:videos/(?P<display_id>[^/]+)_|embed/)(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta name="name" content="No player data">"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://www.pornerbros.com/videos/no-player_181369",
            &context,
        )
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
