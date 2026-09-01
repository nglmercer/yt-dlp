#[test]
fn hytale_native_extractor_builds_cloudflare_playlist() {
    let extractor = HytaleExtractor::new(ExtractorDescriptor::new(
        "HytaleIE",
        "Hytale",
        r#"https?://(?:www\.)?hytale\.com/news/\d+/\d+/(?P<id>[a-z0-9-]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "hytale.com/media".to_owned(),
                br#"<script>window.__INITIAL_COMPONENTS_STATE__ = [{"media":{"clips":[{"src":"0123456789abcdef0123456789abcdef","caption":"Native Hytale clip"}]}}];</script>"#.to_vec(),
            ),
            (
                "hytale.com/news/2021/07/native-update".to_owned(),
                br#"<html><meta property="og:title" content="Native Hytale update"><stream class="ql-video cf-stream" src="0123456789abcdef0123456789abcdef"></stream></html>"#.to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://hytale.com/news/2021/07/native-update",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Hytale playlist");
    };

    assert_eq!(info.get_str("id"), Some("native-update"));
    assert_eq!(info.get_str("title"), Some("Native Hytale update"));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].get_str("_type"), Some("url_transparent"));
    assert_eq!(entries[0].get_str("ie_key"), Some("CloudflareStream"));
    assert_eq!(entries[0].get_str("title"), Some("Native Hytale clip"));
    assert!(entries[0]
        .get_str("url")
        .is_some_and(|url| url.contains("manifest/video.mpd")));
}
