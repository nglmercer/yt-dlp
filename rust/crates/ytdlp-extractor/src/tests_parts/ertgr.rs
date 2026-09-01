#[test]
fn ert_webtv_embed_native_extractor_builds_hls_and_thumbnail() {
    let extractor = ErtWebtvEmbedExtractor::new(ExtractorDescriptor::new(
        "ERTWebtvEmbedIE",
        "ertwebtv:embed",
        r"https?://www\.ert\.gr/webtv/live\-uni/vod/dt\-uni\-vod\.php\?([^#]+&)?f=(?P<id>[^#&]+)",
        true,
    ))
    .unwrap();
    let context = ExtractionContext::new(
        RequestDirector::new(),
        CookieJar::new().shared(),
    );
    let result = extractor
        .extract_with_context(
            "https://www.ert.gr/webtv/live-uni/vod/dt-uni-vod.php?f=trailers/E2251.mp4&bgimg=/photos/native.jpg",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("trailers/E2251.mp4"));
    assert_eq!(
        result.get_str("title"),
        Some("VOD - trailers/E2251.mp4")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://program.ert.gr/photos/native.jpg")
    );
    assert_eq!(
        result.get_str("url"),
        Some("https://mediastream.ert.gr/vodedge/_definst_/mp4:dvrorigin/trailers/E2251.mp4/playlist.m3u8")
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn ert_webtv_embed_native_extractor_keeps_absolute_thumbnail() {
    let extractor = ErtWebtvEmbedExtractor::new(ExtractorDescriptor::new(
        "ERTWebtvEmbedIE",
        "ertwebtv:embed",
        r"https?://www\.ert\.gr/webtv/live\-uni/vod/dt\-uni\-vod\.php\?([^#]+&)?f=(?P<id>[^#&]+)",
        true,
    ))
    .unwrap();
    let context = ExtractionContext::new(
        RequestDirector::new(),
        CookieJar::new().shared(),
    );
    let result = extractor
        .extract_with_context(
            "https://www.ert.gr/webtv/live-uni/vod/dt-uni-vod.php?f=sample.mp4&bgimg=https://images.example/native.jpg",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://images.example/native.jpg")
    );
}
