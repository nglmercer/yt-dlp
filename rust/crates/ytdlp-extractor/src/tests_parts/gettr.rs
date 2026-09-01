#[test]
fn gettr_native_extractor_maps_post_hls_and_metadata() {
    let extractor = GettrExtractor::new(ExtractorDescriptor::new(
        "GettrIE",
        "Gettr",
        r#"https?://(www\.)?gettr\.com/post/(?P<id>[a-z0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "gettr.com/post/pcf6uv838f".to_owned(),
                br#"<meta property="og:description" content="API fallback description">
                    <meta property="og:image" content="https://cdn.example/poster.jpg">"#
                    .to_vec(),
            ),
            (
                "api.gettr.com/u/post/pcf6uv838f".to_owned(),
                br#"{"result":{"data":{"txt":"Native GETTR post","uid":"epochtv",
                    "vid":"video/pcf6uv838f.m3u8","main":"out.jpg","cdate":1632782451058,
                    "vid_dur":58.5585,"htgs":["hornofafrica","explorations"]},
                    "aux":{"uinf":{"epochtv":{"nickname":"EpochTV","_id":"epochtv"}}}}}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.gettr.com/post/pcf6uv838f", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("pcf6uv838f"));
    assert_eq!(
        result.get_str("title"),
        Some("EpochTV - Native GETTR post")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native GETTR post")
    );
    assert_eq!(result.get_str("uploader"), Some("EpochTV"));
    assert_eq!(result.get_str("uploader_id"), Some("epochtv"));
    assert_eq!(result.get_f64("timestamp"), Some(1_632_782_451.058));
    assert_eq!(result.get_f64("duration"), Some(58.5585));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://media.gettr.com/out.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://media.gettr.com/video/pcf6uv838f.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn gettr_native_extractor_preserves_quote_post_redirects() {
    let extractor = GettrExtractor::new(ExtractorDescriptor::new(
        "GettrIE",
        "Gettr",
        r#"https?://(www\.)?gettr\.com/post/(?P<id>[a-z0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "gettr.com/post/quote123".to_owned(),
                br#"<title>Quote on GETTR</title>"#.to_vec(),
            ),
            (
                "api.gettr.com/u/post/quote123".to_owned(),
                br#"{"result":{"data":{"prevsrc":"https://cdn.example/embed/quote123"}}}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://gettr.com/post/quote123", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/embed/quote123")
    );
}

#[test]
fn gettr_streaming_native_extractor_posts_join_request() {
    let extractor = GettrStreamingExtractor::new(ExtractorDescriptor::new(
        "GettrStreamingIE",
        "GettrStreaming",
        r#"https?://(www\.)?gettr\.com/streaming/(?P<id>[a-z0-9]+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "api.gettr.com/u/live/join/psoiulc122".to_owned(),
            br#"{"result":{"broadcast":{"url":"https://cdn.example/live/master.m3u8",
                "viewsCount":12,"startAt":1644080997164,"duration":5180184,
                "isLive":true},"postData":{"ttl":"Native live title",
                "dsc":"Native live description","imgs":["live.jpg"]},
                "liveHostInfo":{"nickname":"Native Host","_id":"host-1"}}}"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://gettr.com/streaming/psoiulc122",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("psoiulc122"));
    assert_eq!(result.get_str("title"), Some("Native live title"));
    assert_eq!(result.get_str("uploader"), Some("Native Host"));
    assert_eq!(result.get_i64("view_count"), Some(12));
    assert_eq!(result.get_f64("timestamp"), Some(1_644_080_997.164));
    assert_eq!(result.get_f64("duration"), Some(5_180.184));
    assert_eq!(result.get_bool("is_live"), Some(true));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/live/master.m3u8")
    );
    assert_eq!(
        result
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
            .and_then(|values| values.first())
            .and_then(|value| value.get("url"))
            .and_then(serde_json::Value::as_str),
        Some("https://media.gettr.com/live.jpg")
    );
}
