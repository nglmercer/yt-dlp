#[test]
fn formula1_native_extractor_redirects_to_brightcove() {
    let extractor = Formula1Extractor::new(ExtractorDescriptor::new(
        "Formula1IE",
        "Formula1",
        r#"https?://(?:www\.)?formula1\.com/en/latest/video\.[^.]+\.(?P<id>\d+)\.html"#,
        true,
    ))
    .unwrap();
    let context = ExtractionContext::native();
    let ExtractorResult::Redirect { url, ie_key } = extractor
        .extract_with_context(
            "https://www.formula1.com/en/latest/video.race-highlights-spain-2016.6060988138001.html",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Formula 1 redirect");
    };

    assert_eq!(
        url,
        "http://players.brightcove.net/6057949432001/S1WMrhjlh_default/index.html?videoId=6060988138001"
    );
    assert_eq!(ie_key.as_deref(), Some("BrightcoveNew"));
}
