#[test]
fn iltalehti_native_extractor_maps_embedded_jwplatform_playlist() {
    let extractor = IltalehtiExtractor::new(ExtractorDescriptor::new(
        "IltalehtiIE",
        "Iltalehti",
        r#"https?://(?:www\.)?iltalehti\.fi/[^/?#]+/a/(?P<id>[^/?#])"#,
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.iltalehti.fi/politiikka/a/n".to_owned(),
            br#"<script>window.App = {
                "state":{"articles":{"article":{
                    "canonical_title":"Native Iltalehti article",
                    "items":[
                        {"main_media":{"properties":{"provider":"jwplayer","id":"gYjjaf1L"}}},
                        {"body":[{"properties":{"provider":"jwplayer","id":"18R6zkLi"}}]}
                    ]
                }}}
            };</script>"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.iltalehti.fi/politiikka/a/n",
            &context,
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected Iltalehti playlist");
    };
    assert_eq!(info.get_str("id"), Some("n"));
    assert_eq!(
        info.get_str("title"),
        Some("Native Iltalehti article")
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get_str("url"),
        Some("jwplatform:18R6zkLi")
    );
    assert_eq!(
        entries[0].get_str("ie_key"),
        Some("JWPlatform")
    );
}
