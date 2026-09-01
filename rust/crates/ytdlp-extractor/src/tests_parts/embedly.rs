#[test]
fn embedly_native_extractor_redirects_youtube_playlist_target() {
    let extractor = EmbedlyExtractor::new(ExtractorDescriptor::new(
        "EmbedlyIE",
        "Embedly",
        r"https?://(?:www|cdn\.)?embedly\.com/widgets/media\.html\?(?:[^#]*?&)?(?:src|url)=(?:[^#&]+)",
        true,
    ))
    .unwrap();
    let widget_url = "https://cdn.embedly.com/widgets/media.html?src=https%3A%2F%2Fwww.youtube.com%2Fembed%2Fvideoseries%3Flist%3DUUGLim4T2loE5rwCMdpCIPVg&url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DSU4fj_aEMVw%26list%3DUUGLim4T2loE5rwCMdpCIPVg";
    assert_eq!(
        extractor.extract_with_context(widget_url, &ExtractionContext::native()),
        Ok(ExtractorResult::Redirect {
            url: "https://www.youtube.com/watch?v=SU4fj_aEMVw&list=UUGLim4T2loE5rwCMdpCIPVg"
                .to_owned(),
            ie_key: Some("YoutubeTab".to_owned()),
        })
    );
}

#[test]
fn embedly_native_extractor_preserves_referer_for_non_youtube_target() {
    let extractor = EmbedlyExtractor::new(ExtractorDescriptor::new(
        "EmbedlyIE",
        "Embedly",
        r"https?://(?:www|cdn\.)?embedly\.com/widgets/media\.html\?(?:[^#]*?&)?(?:src|url)=(?:[^#&]+)",
        true,
    ))
    .unwrap();
    let widget_url =
        "https://www.embedly.com/widgets/media.html?src=https%3A%2F%2Fplayer.vimeo.com%2Fvideo%2F1234567";
    let result = extractor
        .extract_with_context(widget_url, &ExtractionContext::native())
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("url"),
        Some("https://player.vimeo.com/video/1234567")
    );
    assert_eq!(
        result
            .get("http_headers")
            .and_then(|headers| headers.get("Referer"))
            .and_then(serde_json::Value::as_str),
        Some(widget_url)
    );
}
