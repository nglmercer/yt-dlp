#[test]
fn imdb_native_extractor_maps_next_data_playback_urls() {
    let extractor = ImdbExtractor::new(ExtractorDescriptor::new(
        "ImdbIE",
        "imdb",
        r#"https?://(?:www|m)\.imdb\.com/(?:video|title|list).*?[/-]vi(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.imdb.com/video/vi2524815897".to_owned(),
            br#"<html><head><title>Native IMDb</title></head><body>
                <script id="__NEXT_DATA__">{
                    "videoSubTitle":"Native subtitle",
                    "props":{"pageProps":{
                        "videoPlaybackData":{"video":{
                            "name":{"value":"Native IMDb trailer"},
                            "description":{"value":"Native IMDb description"},
                            "thumbnail":{"url":"https://cdn.example/imdb.jpg"},
                            "runtime":{"value":152},
                            "playbackURLs":[
                                {"url":"https://cdn.example/imdb.mp4","mimeType":"video/mp4","displayName":{"value":"720p"}},
                                {"url":"https://cdn.example/imdb.m3u8","mimeType":"application/x-mpegURL","displayName":{"value":"HLS"}}
                            ]
                        }}
                    }}
                }</script>
            </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.imdb.com/video/imdb/vi2524815897",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("2524815897"));
    assert_eq!(result.get_str("title"), Some("Native IMDb trailer"));
    assert_eq!(
        result.get_str("description"),
        Some("Native IMDb description")
    );
    assert_eq!(result.get_str("alt_title"), Some("Native subtitle"));
    assert_eq!(result.get("duration"), Some(&serde_json::json!(152.0)));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0].get("quality"), Some(&serde_json::json!(2)));
    assert_eq!(
        formats[1].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn imdb_native_extractor_uses_legacy_playback_api_when_needed() {
    let extractor = ImdbExtractor::new(ExtractorDescriptor::new(
        "ImdbIE",
        "imdb",
        r#"https?://(?:www|m)\.imdb\.com/(?:video|title|list).*?[/-]vi(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "www.imdb.com/video/vi3516832537".to_owned(),
                br#"<script id="__NEXT_DATA__">{
                    "props":{"pageProps":{"videoPlaybackData":{"video":{
                        "name":{"value":"Legacy IMDb trailer"},
                        "playbackURLs":[]
                    }}}}
                }</script>"#
                    .to_vec(),
            ),
            (
                "www.imdb.com/ve/data/VIDEO_PLAYBACK_DATA".to_owned(),
                br#"[{"videoLegacyEncodings":[{
                    "url":"https://cdn.example/legacy.mp4",
                    "mimeType":"video/mp4",
                    "definition":"480p"
                }]}]"#
                .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.imdb.com/video/vi3516832537",
            &context,
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("3516832537"));
    assert_eq!(result.get_str("title"), Some("Legacy IMDb trailer"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/legacy.mp4")
    );
}
