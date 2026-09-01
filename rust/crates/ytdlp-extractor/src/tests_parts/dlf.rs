#[test]
fn dlf_native_extractor_maps_hls_button_attributes() {
    let extractor = DlfExtractor::new(ExtractorDescriptor::new(
        "DLFIE",
        "dlf",
        r"https?://(?:www\.)?deutschlandfunk\.de/[\w-]+-dlf-(?P<id>[\da-f]{8})-100\.html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: r#"<button alt="Anhören" data-audio-diraid="native-dlf"
            data-audiotitle="Native DLF audio" data-audioduration="3298"
            data-audioimage="https://cdn.example/dlf.jpg"
            data-audio-producer="Deutschlandfunk" data-audio-series="On Stage"
            data-audio-origin-site-name="deutschlandfunk"
            data-audio-download-tracking-path="https://www.deutschlandfunk.de/native"
            data-audio-download-src="https://cdn.example/native.m3u8">"#
            .as_bytes()
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.deutschlandfunk.de/native-dlf-03a3eb19-100.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("03a3eb19"));
    assert_eq!(result.get_str("title"), Some("Native DLF audio"));
    assert_eq!(result.get_i64("duration"), Some(3298));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/dlf.jpg"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/native.m3u8"));
    assert_eq!(result.get_str("ext"), Some("m4a"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("protocol")),
        Some(&serde_json::json!("m3u8_native"))
    );
}

#[test]
fn dlf_corpus_native_extractor_maps_audio_button_playlist() {
    let extractor = DlfExtractor::new(ExtractorDescriptor::new(
        "DLFCorpusIE",
        "dlf:corpus",
        r"https?://(?:www\.)?deutschlandfunk\.de/(?P<id>(?![\w-]+-dlf-[\da-f]{8})[\w-]+-\d+)\.html",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: r#"<head>
            <meta property="og:title" content="Native corpus">
            <meta name="description" content="Native corpus description">
        </head><body>
            <button alt="Anhören" data-audio-diraid="first"
                data-audio-title="First item" data-audio="https://cdn.example/first.mp3">
            <button alt="Nicht anhören" data-audio-diraid="ignored"
                data-audio="https://cdn.example/ignored.mp3">
            <button alt="Anhören" data-audio-diraid="second"
                data-audio-download-src="https://cdn.example/second.ogg">
        </body>"#
            .as_bytes()
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.deutschlandfunk.de/native-corpus-100.html",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Deutschlandfunk corpus playlist");
    };

    assert_eq!(info.get_str("id"), Some("native-corpus-100"));
    assert_eq!(info.get_str("title"), Some("Native corpus"));
    assert_eq!(
        info.get_str("description"),
        Some("Native corpus description")
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("first"));
    assert_eq!(entries[0].get_str("title"), Some("First item"));
    assert_eq!(entries[1].get_str("url"), Some("https://cdn.example/second.ogg"));
}
