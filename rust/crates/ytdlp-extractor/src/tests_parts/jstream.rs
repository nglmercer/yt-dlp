#[test]
fn jstream_native_extractor_maps_jsonp_auto_hls_renditions() {
    let extractor = JstreamExtractor::new(ExtractorDescriptor::new(
        "JStreamIE",
        "JStream",
        r#"jstream:(?P<host>www\d+):(?P<id>(?P<publisher>[a-z0-9]+):(?P<mid>\d+))"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "eqd638pvwx.eq.webcdn.stream.ne.jp/www50/eqd638pvwx/jmc_pub/eq_meta/v1/752.jsonp"
                .to_owned(),
            br#"metaDataResult({"movie":{
                "title":"Native JStream title",
                "duration":672,
                "thumbnail_url":"https://cdn.example/jstream.jpg",
                "movie_list_hls":[
                    {"text":"auto_720p","url":"video/752-720.m3u8"},
                    {"text":"manual","url":"video/752-manual.m3u8"}
                ]
            }})"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("jstream:www50:eqd638pvwx:752", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("eqd638pvwx:752"));
    assert_eq!(result.get_str("title"), Some("Native JStream title"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(672.0)));
    assert_eq!(
        result.get_str("url"),
        Some("https://eqd638pvwx.eq.webcdn.stream.ne.jp/www50/eqd638pvwx/jmc_pub/video/752-720.m3u8")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 1);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("720p")));
}
