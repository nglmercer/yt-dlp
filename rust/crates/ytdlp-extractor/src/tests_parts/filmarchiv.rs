#[test]
fn filmarchiv_native_extractor_builds_cdn_hls_and_page_metadata() {
    let extractor = FilmArchivExtractor::new(ExtractorDescriptor::new(
        "FilmArchivIE",
        "FilmArchiv",
        r"https?://(?:www\.)?filmarchiv\.at/de/filmarchiv-on/video/(?P<id>f_[0-9a-zA-Z]{5,})",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "www.filmarchiv.at/de/filmarchiv-on/video/f_0305p7xKrXUPBwoNE9x6mh".to_owned(),
            r#"<html><head>
                    <meta name="description" content="Fallback description">
                </head><body>
                    <title-div>Der Wurstelprater zur Kaiserzeit</title-div>
                    <div class="border-base-content"><div class="prose">Native archive description</div></div>
                </body></html>"#
                .as_bytes()
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.filmarchiv.at/de/filmarchiv-on/video/f_0305p7xKrXUPBwoNE9x6mh",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("f_0305p7xKrXUPBwoNE9x6mh"));
    assert_eq!(
        result.get_str("title"),
        Some("Der Wurstelprater zur Kaiserzeit")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native archive description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.filmarchiv.at/f_0305/p7xKrXUPBwoNE9x6mh_v1/poster.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.filmarchiv.at/f_0305/p7xKrXUPBwoNE9x6mh_v1_sv1/playlist.m3u8")
    );
}
