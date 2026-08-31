#[test]
fn altcensored_native_extractor_returns_archive_transparent_result() {
    let extractor = AltCensoredExtractor::new(ExtractorDescriptor::new(
        "AltCensoredIE",
        "altcensored",
        r"https?://(?:www\.)?altcensored\.com/(?:watch\?v=|embed/)(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![(
            "altcensored.com/watch?v=k0srjLSkga8".to_owned(),
            br#"<a href="/category/42">News &amp; Politics</a>
                YouTube Views:&nbsp;12,345"#
                .to_vec(),
        )],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.altcensored.com/watch?v=k0srjLSkga8",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(
        result.get_str("url"),
        Some("https://archive.org/details/youtube-k0srjLSkga8")
    );
    assert_eq!(result.get_str("ie_key"), Some("ArchiveOrg"));
    assert_eq!(result.get_i64("view_count"), Some(12_345));
    assert_eq!(
        result.get("categories"),
        Some(&serde_json::json!(["News & Politics"]))
    );
}

#[test]
fn altcensored_channel_native_extractor_materializes_deduplicated_pages() {
    let extractor = AltCensoredChannelExtractor::new(ExtractorDescriptor::new(
        "AltCensoredChannelIE",
        "altcensored:channel",
        r"https?://(?:www\.)?altcensored\.com/channel/(?!page|table)(?P<id>[^/?#]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "altcensored.com/channel/UCnative/page/1".to_owned(),
                br#"<a href="/watch?v=first"><span>First</span></a>
                    <a href="/watch?v=duplicate">Duplicate</a>"#
                    .to_vec(),
            ),
            (
                "altcensored.com/channel/UCnative/page/2".to_owned(),
                br#"<a href="/watch?v=duplicate">Duplicate</a>
                    <a href="/watch?v=second">Second</a>"#
                    .to_vec(),
            ),
            (
                "altcensored.com/channel/UCnative".to_owned(),
                br#"<meta name="altcen_title" content="Native channel">
                    <a href="/channel/UCnative/page/2">2</a>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.altcensored.com/channel/UCnative",
            &context,
        )
        .unwrap()
    else {
        panic!("expected AltCensored channel playlist");
    };

    assert_eq!(info.get_str("id"), Some("UCnative"));
    assert_eq!(info.get_str("title"), Some("Native channel"));
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].get_str("ie_key"), Some("AltCensored"));
    assert_eq!(
        entries[2].get_str("url"),
        Some("https://www.altcensored.com/watch?v=second")
    );
}
