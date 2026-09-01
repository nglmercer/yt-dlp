struct LifeNewsHandler;

impl RequestHandler for LifeNewsHandler {
    fn name(&self) -> &str {
        "lifenews-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("/t/news/98736") {
            let page = r#"
                <meta property="og:title" content="Native Life title - Life.ru">
                <meta property="og:description" content="Native Life description">
                <div class="hits-count">42</div>
                <time datetime="2012-08-05T10:59:00Z"></time>
                <video class="player"><source src="/media/native-life.mp4"></video>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        if url.contains("/t/news/152125") {
            let page = r#"
                <meta property="og:title" content="Native Life iframe - Life.ru">
                <meta property="og:description" content="Iframe description">
                <time datetime="2015-04-02T12:04:00Z"></time>
                <iframe src="//embed.life.ru/embed/e50c2dec2867350528e2574c899b8291"></iframe>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        if url.contains("/t/news/153461") {
            let page = r#"
                <meta property="og:title" content="Native Life playlist - Life.ru">
                <meta property="og:description" content="Playlist description">
                <div class="hits-count">99</div>
                <video><source src="/media/one.mp4"></video>
                <iframe src="https://embed.life.ru/video/e50c2dec2867350528e2574c899b8291"></iframe>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        if url.contains("embed.life.ru") {
            let page = r#"
                <script>
                    options = {
                        playlist: {
                            master: "https://cdn.example/life/master.m3u8",
                            original: "https://cdn.example/life/original.mp4",
                            image: "https://cdn.example/life/poster.jpg"
                        }
                    };
                </script>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Life.ru route for {url}"),
        ))
    }
}

fn lifenews_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(LifeNewsHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

fn lifenews_extractor() -> LifeNewsExtractor {
    LifeNewsExtractor::new(
        ExtractorDescriptor::new(
            "LifeNewsIE",
            "life",
            r#"https?://life\.ru/t/[^/]+/(?P<id>\d+)"#,
            true,
        ),
    )
    .unwrap()
}

fn life_embed_extractor() -> LifeEmbedExtractor {
    LifeEmbedExtractor::new(
        ExtractorDescriptor::new(
            "LifeEmbedIE",
            "life:embed",
            r#"https?://embed\.life\.ru/(?:embed|video)/(?P<id>[\da-f]{32})"#,
            true,
        ),
    )
    .unwrap()
}

#[test]
fn lifenews_native_extractor_maps_direct_article_media() {
    let result = lifenews_extractor()
        .extract_with_context(
            "https://life.ru/t/news/98736",
            &lifenews_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("98736"));
    assert_eq!(result.get_str("title"), Some("Native Life title"));
    assert_eq!(result.get_str("description"), Some("Native Life description"));
    assert_eq!(result.get_i64("view_count"), Some(42));
    assert_eq!(result.get_i64("timestamp"), Some(1_344_164_340));
    assert_eq!(
        result.get_str("url"),
        Some("https://life.ru/media/native-life.mp4")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}

#[test]
fn lifenews_native_extractor_delegates_single_iframe_transparently() {
    let result = lifenews_extractor()
        .extract_with_context(
            "https://life.ru/t/news/152125",
            &lifenews_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(result.get_str("ie_key"), Some("LifeEmbed"));
    assert_eq!(
        result.get_str("url"),
        Some("http://embed.life.ru/embed/e50c2dec2867350528e2574c899b8291")
    );
    assert_eq!(result.get_str("id"), Some("152125"));
}

#[test]
fn lifenews_native_extractor_builds_mixed_playlist() {
    let result = lifenews_extractor()
        .extract_with_context(
            "https://life.ru/t/news/153461",
            &lifenews_context(),
        )
        .unwrap();
    let ExtractorResult::Playlist { info, entries } = result else {
        panic!("expected Life.ru playlist");
    };
    assert_eq!(info.get_str("id"), Some("153461"));
    assert_eq!(info.get_str("title"), Some("Native Life playlist"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("153461-video1"));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://life.ru/media/one.mp4")
    );
    assert_eq!(entries[1].get_str("_type"), Some("url_transparent"));
    assert_eq!(entries[1].get_str("ie_key"), Some("LifeEmbed"));
}

#[test]
fn life_embed_native_extractor_maps_master_and_original_media() {
    let result = life_embed_extractor()
        .extract_with_context(
            "https://embed.life.ru/video/e50c2dec2867350528e2574c899b8291",
            &lifenews_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/life/poster.jpg")
    );
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 2);
    assert_eq!(
        formats[0].get("url"),
        Some(&serde_json::json!("https://cdn.example/life/master.m3u8"))
    );
    assert_eq!(
        formats[0].get("protocol"),
        Some(&serde_json::json!("m3u8_native"))
    );
    assert_eq!(
        formats[1].get("url"),
        Some(&serde_json::json!("https://cdn.example/life/original.mp4"))
    );
}
