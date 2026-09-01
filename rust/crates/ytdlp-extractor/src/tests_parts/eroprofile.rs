#[test]
fn eroprofile_native_extractor_maps_html5_video_and_metadata() {
    let extractor = EroProfileExtractor::new(ExtractorDescriptor::new(
        "EroProfileIE",
        "EroProfile",
        r"https?://(?:www\.)?eroprofile\.com/m/videos/view/(?P<id>[^/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html><table><tr><th>Title:</th><td>Native <b>profile</b> video</td></tr></table>
            <script>glbUpdViews('1','3733775');</script>
            <video poster="/images/native.jpg"><source src="/media/native.m4v" type="video/mp4"></video>
        </html>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.eroprofile.com/m/videos/view/sexy-babe-softcore",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("3733775"));
    assert_eq!(
        result.get_str("display_id"),
        Some("sexy-babe-softcore")
    );
    assert_eq!(result.get_str("title"), Some("Native profile video"));
    assert_eq!(result.get_i64("age_limit"), Some(18));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://www.eroprofile.com/images/native.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://www.eroprofile.com/media/native.m4v")
    );
    assert_eq!(result.get_str("ext"), Some("m4v"));
}

#[test]
fn eroprofile_native_extractor_marks_authenticated_pages_as_todo() {
    let extractor = EroProfileExtractor::new(ExtractorDescriptor::new(
        "EroProfileIE",
        "EroProfile",
        r"https?://(?:www\.)?eroprofile\.com/m/videos/view/(?P<id>[^/]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: b"<p>You must be logged in to view this video.</p>".to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let error = extractor
        .extract_with_context(
            "https://www.eroprofile.com/m/videos/view/secret-video",
            &context,
        )
        .unwrap_err();

    assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
    assert!(error.message.starts_with("TODO:"));
}
