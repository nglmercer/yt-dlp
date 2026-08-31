#[test]
fn baidu_video_native_extractor_maps_api_playlist() {
    let extractor = BaiduVideoExtractor::new(ExtractorDescriptor::new(
        "BaiduVideoIE",
        "BaiduVideo",
        r"https?://v\.baidu\.com/(?P<type>[a-z]+)/(?P<id>\d+)\.htm",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "app.video.baidu.com/xqinfo/?worktype=adnativetvshow&id=11595".to_owned(),
                br#"{"title":"Native Baidu Show","intro":"Native &amp; detailed intro"}"#.to_vec(),
            ),
            (
                "app.video.baidu.com/xqsingle/?worktype=adnativetvshow&id=11595".to_owned(),
                br#"{"videos":[
                    {"url":"https://v.baidu.com/episode/native-1","title":"Episode 1"},
                    {"url":"https://v.baidu.com/episode/native-2","title":"Episode 2"}
                ]}"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let extraction = extractor
        .extract_with_context(
            "http://v.baidu.com/show/11595.htm?frp=bdbrand",
            &context,
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = extraction else {
        panic!("expected Baidu playlist result");
    };

    assert_eq!(info.get_str("id"), Some("11595"));
    assert_eq!(info.get_str("title"), Some("Native Baidu Show"));
    assert_eq!(
        info.get_str("description"),
        Some("Native & detailed intro")
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("_type"), Some("url"));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://v.baidu.com/episode/native-1")
    );
    assert_eq!(entries[0].get_str("title"), Some("Episode 1"));
}
