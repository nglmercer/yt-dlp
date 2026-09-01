#[test]
fn gaskrank_native_extractor_maps_html5_media_and_metadata() {
    let extractor = GaskrankExtractor::new(ExtractorDescriptor::new(
        "GaskrankIE",
        "Gaskrank",
        r#"https?://(?:www\.)?gaskrank\.tv/tv/(?P<categories>[^/]+)/(?P<id>[^/]+)\.htm"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><head>
            <meta property="og:title" content="Native Gaskrank title">
        </head><body>
            <video><source src="https://movies.gaskrank.tv/201601/26955-crash.mp4" type="video/mp4"></video>
            Video von: Bikefun | vom: 10.01.2017
            Homepage: <a>www.iomtt.com</a>
            <a href="/tv/tags/racing/">Racing</a>
            <span class="gkRight"><i class="icon-eye-open"></i> 1.234</span>
            <span itemprop="ratingValue">4,5</span>
        </body></html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.gaskrank.tv/tv/racing/native-gaskrank.htm",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("201601/26955"));
    assert_eq!(result.get_str("title"), Some("Native Gaskrank title"));
    assert_eq!(result.get_str("display_id"), Some("native-gaskrank"));
    assert_eq!(result.get_str("upload_date"), Some("20170110"));
    assert_eq!(result.get_str("uploader_id"), Some("Bikefun"));
    assert_eq!(result.get_str("uploader_url"), Some("www.iomtt.com"));
    assert_eq!(result.get_i64("view_count"), Some(1_234));
    assert_eq!(result.get_f64("average_rating"), Some(4.5));
    assert_eq!(
        result
            .get("categories")
            .and_then(serde_json::Value::as_array)
            .and_then(|values| values.first())
            .and_then(serde_json::Value::as_str),
        Some("racing")
    );
    assert_eq!(
        result
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .and_then(|values| values.first())
            .and_then(serde_json::Value::as_str),
        Some("Racing")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://movies.gaskrank.tv/201601/26955-crash.mp4")
    );
}

#[test]
fn gaskrank_native_extractor_falls_back_to_embedded_movie_url() {
    let extractor = GaskrankExtractor::new(ExtractorDescriptor::new(
        "GaskrankIE",
        "Gaskrank",
        r#"https?://(?:www\.)?gaskrank\.tv/tv/(?P<categories>[^/]+)/(?P<id>[^/]+)\.htm"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<meta name="title" content="Fallback title">
            <script>var movie = "https://movies.gaskrank.tv/202001/12345.mp4";</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.gaskrank.tv/tv/motorrad-fun/fallback.htm",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("202001/12345"));
    assert_eq!(
        result.get_str("url"),
        Some("https://movies.gaskrank.tv/202001/12345.mp4")
    );
}
