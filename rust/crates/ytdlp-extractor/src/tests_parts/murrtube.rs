#[test]
fn murrtube_native_extractor_initializes_age_session_and_maps_hls_metadata() {
    let extractor = MurrtubeExtractor::new(ExtractorDescriptor::new(
        "MurrtubeIE",
        "Murrtube",
        r#"(?x)(?:murrtube:|https?://murrtube\.net/(?:v/|videos/(?P<slug>[a-z0-9-]+?)-))(?P<id>[A-Z0-9]{4}|[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "murrtube.net/accept_age_check".to_owned(),
                b"age accepted".to_vec(),
            ),
            (
                "murrtube.net/videos/inferno-x-skyler".to_owned(),
                r#"<html>
                    <meta property="og:title" content="Inferno X Skyler - Murrtube">
                    <meta property="og:description" content="Native Murrtube description">
                    <meta property="og:image" content="https://storage.murrtube.net/thumb.jpg?size=large">
                    <div id="video" data-url="https://storage.murrtube.net/ca885d8456b95de529b6723b158032e11115d/index.m3u8?token=native"></div>
                    <span>1,234 <span>Views</span></span>
                    <span>56 <span>Likes</span></span>
                    <span>7 <span>Comments</span></span>
                    <div class="pl-1 is-size-6 has-text-lighter">Inferno Wolf</div>
                </html>"#
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "murrtube.net".to_owned(),
                b"<input type=\"hidden\" name=\"age\" value=\"18\">".to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://murrtube.net/videos/inferno-x-skyler-148b6f2a-fdcc-4902-affe-9c0f41aaaca0",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("id"),
        Some("ca885d8456b95de529b6723b158032e11115d")
    );
    assert_eq!(result.get_str("title"), Some("Inferno X Skyler"));
    assert_eq!(result.get_i64("age_limit"), Some(18));
    assert_eq!(result.get_str("uploader"), Some("Inferno Wolf"));
    assert_eq!(result.get_i64("view_count"), Some(1234));
    assert_eq!(result.get_i64("like_count"), Some(56));
    assert_eq!(result.get_i64("comment_count"), Some(7));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://storage.murrtube.net/thumb.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://storage.murrtube.net/ca885d8456b95de529b6723b158032e11115d/index.m3u8")
    );
}
