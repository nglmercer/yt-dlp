#[test]
fn hse_show_native_extractor_maps_redux_video_state() {
    let extractor = HseShowExtractor::new(ExtractorDescriptor::new(
        "HSEShowIE",
        "HSEShow",
        r#"https?://(?:www\.)?hse\.de/dpl/c/tv-shows/(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>window.__REDUX_DATA__ = {
            "tvShowPage": {
                "tvShow": {
                    "title": "Native HSE show",
                    "date": "2021-12-06",
                    "hour": "09",
                    "actionFieldText": "tvShow | HSE24_demo",
                    "presenter": "Native Presenter"
                },
                "tvShowVideo": {
                    "poster": "https://cdn.example/hse-show.jpg",
                    "sources": [{
                        "mimetype": "application/x-mpegURL",
                        "url": "https://cdn.example/hse-show.m3u8",
                        "subtitles": {
                            "de": [{"url": "https://cdn.example/hse-show.vtt"}]
                        }
                    }]
                }
            }
        };</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.hse.de/dpl/c/tv-shows/505350", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("505350"));
    assert_eq!(result.get_str("title"), Some("Native HSE show"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/hse-show.m3u8"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get("timestamp"), Some(&serde_json::json!(1638781200)));
    assert_eq!(result.get_str("channel"), Some("HSE24"));
    assert_eq!(result.get_str("uploader"), Some("Native Presenter"));
    assert_eq!(
        result
            .get("subtitles")
            .and_then(|subtitles| subtitles.get("de"))
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("url")),
        Some(&serde_json::json!("https://cdn.example/hse-show.vtt"))
    );
}

#[test]
fn hse_product_native_extractor_maps_product_video_state() {
    let extractor = HseProductExtractor::new(ExtractorDescriptor::new(
        "HSEProductIE",
        "HSEProduct",
        r#"https?://(?:www\.)?hse\.de/dpl/p/product/(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>window.__REDUX_DATA__ = {
            "productContent": {
                "productContent": {
                    "videos": [{
                        "poster": "https://cdn.example/hse-product.jpg",
                        "sources": [{
                            "mimetype": "application/x-mpegURL",
                            "url": "https://cdn.example/hse-product.m3u8"
                        }]
                    }]
                }
            },
            "productDetail": {
                "product": {
                    "name": {"short": "Native HSE product"},
                    "brand": {"brandName": "Native Brand"}
                }
            }
        };</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.hse.de/dpl/p/product/408630", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("408630"));
    assert_eq!(result.get_str("title"), Some("Native HSE product"));
    assert_eq!(result.get_str("uploader"), Some("Native Brand"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/hse-product.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/hse-product.m3u8")
    );
}

#[test]
fn hse_native_extractor_marks_non_hls_sources_as_todo() {
    let extractor = HseShowExtractor::new(ExtractorDescriptor::new(
        "HSEShowIE",
        "HSEShow",
        r#"https?://(?:www\.)?hse\.de/dpl/c/tv-shows/(?P<id>[0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script>window.__REDUX_DATA__ = {
            "tvShowPage": {
                "tvShow": {"title": "Native HSE show"},
                "tvShowVideo": {
                    "sources": [{
                        "mimetype": "video/mp4",
                        "url": "https://cdn.example/hse-encrypted.mp4"
                    }]
                }
            }
        };</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context("https://www.hse.de/dpl/c/tv-shows/505350", &context)
        .unwrap_err();
    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
