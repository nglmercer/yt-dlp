#[test]
fn fathom_native_extractor_maps_page_state_and_hls() {
    let extractor = FathomExtractor::new(ExtractorDescriptor::new(
        "FathomIE",
        "Fathom",
        r"https?://(?:www\.)?fathom\.video/share/(?P<id>[^/?#&]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "fathom.video/share/G9mkjkspnohVVZ_L5nrsoPycyWcB8y7s".to_owned(),
            br#"<html><body>
                <div data-page="{&quot;props&quot;:{
                    &quot;call&quot;:{
                        &quot;id&quot;:47200596,
                        &quot;video_url&quot;:&quot;https://cdn.example/fathom/47200596/master.m3u8&quot;,
                        &quot;started_at&quot;:&quot;2023-11-03T12:00:00Z&quot;
                    },
                    &quot;head&quot;:{&quot;title&quot;:&quot;eCom Incubator - Coaching Session&quot;},
                    &quot;duration&quot;:8125.380507
                }}" id="app"></div>
            </body></html>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://fathom.video/share/G9mkjkspnohVVZ_L5nrsoPycyWcB8y7s",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("47200596"));
    assert_eq!(
        result.get_str("title"),
        Some("eCom Incubator - Coaching Session")
    );
    assert_eq!(result.get_f64("duration"), Some(8125.380507));
    assert_eq!(result.get_i64("timestamp"), Some(1699012800));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/fathom/47200596/master.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol"))
            .and_then(serde_json::Value::as_str),
        Some("m3u8_native")
    );
}
