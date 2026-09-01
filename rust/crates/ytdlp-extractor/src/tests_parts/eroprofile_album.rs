#[test]
fn eroprofile_album_native_extractor_paginates_and_builds_native_entries() {
    let extractor = EroProfileAlbumExtractor::new(ExtractorDescriptor::new(
        "EroProfileAlbumIE",
        "EroProfile:album",
        r"https?://(?:www\.)?eroprofile\.com/m/videos/album/(?P<id>[^/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "album/BBW-2-893?pnum=2".to_owned(),
                br#"<a href="/m/videos/view/second-video"></a>
                    <a href="/m/videos/view/first-video"></a>"#
                    .to_vec(),
            ),
            (
                "album/BBW-2-893?pnum=3".to_owned(),
                br#"<a href="/m/videos/view/third-video"></a>"#.to_vec(),
            ),
            (
                "eroprofile.com/m/videos/album/BBW-2-893".to_owned(),
                br#"<title>Album: Native album - EroProfile</title>
                    <a href="/m/videos/view/first-video"></a>
                    <a href="/m/videos/album/BBW-2-893?pnum=2"></a>
                    <a href="/m/videos/album/BBW-2-893?pnum=3"></a>"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.eroprofile.com/m/videos/album/BBW-2-893",
            &context,
        )
        .unwrap()
    else {
        panic!("EroProfile album should return a playlist");
    };

    assert_eq!(info.get_str("id"), Some("BBW-2-893"));
    assert_eq!(info.get_str("title"), Some("Native album"));
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].get_str("ie_key"), Some("EroProfile"));
    assert_eq!(
        entries[1].get_str("url"),
        Some("https://www.eroprofile.com/m/videos/view/second-video")
    );
}
