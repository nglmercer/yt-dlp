struct La7Handler;

impl RequestHandler for La7Handler {
    fn name(&self) -> &str {
        "la7-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("/voicetown/podcast/") {
            let page = r#"
                <div class="title">Native LA7 episode</div>
                <div class="description"><p>Native LA7 description</p></div>
                <div class="podcast-image"><img src="https://cdn.example/la7.jpg"></div>
                <span class="duration">12:34</span>
                <div class="data">23.03.2021</div>
                <script>var player = {src: 'https://cdn.example/native-la7.mp3?format=mp3'};</script>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        if url.ends_with("/propagandalive/podcast") {
            let page = r#"
                <h1>Native LA7 podcast</h1>
                <script>window.ppN = 'Native LA7 podcast';</script>
                <div class="container-podcast-property">
                    data-nid="101" src: 'https://cdn.example/la7-one.mp3'
                    <div class="title">Native LA7 podcast</div>
                    <div class="description">First episode</div>
                    <div class="data">23.03.2021</div>
                </div></div></div>
                <div class="container-podcast-property">
                    data-nid="102" src: 'https://cdn.example/la7-two.mp3'
                    <div class="title">Second episode</div>
                    <div class="description">Second description</div>
                </div></div></div>
            "#;
            return Ok(Response::new(url, 200, "OK", page.as_bytes().to_vec()));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no LA7 route for {url}"),
        ))
    }
}

fn la7_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(La7Handler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn la7_podcast_episode_native_extractor_maps_mp3_metadata() {
    let extractor = La7PodcastEpisodeExtractor::new(ExtractorDescriptor::new(
        "LA7PodcastEpisodeIE",
        "la7.it:pod:episode",
        r#"https?://(?:www\.)?la7\.it/[^/]+/podcast/([^/]+-)?(?P<id>\d+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.la7.it/voicetown/podcast/native-episode-371497",
            &la7_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("371497"));
    assert_eq!(result.get_str("title"), Some("Native LA7 episode"));
    assert_eq!(result.get_str("description"), Some("Native LA7 description"));
    assert_eq!(result.get_str("thumbnail"), Some("https://cdn.example/la7.jpg"));
    assert_eq!(result.get_f64("duration"), Some(754.0));
    assert_eq!(result.get_str("upload_date"), Some("20210323"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.first())
            .and_then(|format| format.get("url")),
        Some(&serde_json::json!("https://cdn.example/native-la7.mp3?format=mp3"))
    );
}

#[test]
fn la7_podcast_native_extractor_builds_episode_entries() {
    let extractor = La7PodcastExtractor::new(ExtractorDescriptor::new(
        "LA7PodcastIE",
        "la7.it:podcast",
        r#"https?://(?:www\.)?la7\.it/(?P<id>[^/]+)/podcast/?(?:$|[#?])"#,
        true,
    ))
    .unwrap();
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://www.la7.it/propagandalive/podcast",
            &la7_context(),
        )
        .unwrap()
    else {
        panic!("expected LA7 podcast playlist");
    };
    assert_eq!(info.get_str("id"), Some("propagandalive"));
    assert_eq!(info.get_str("title"), Some("Native LA7 podcast"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("id"), Some("101"));
    assert_eq!(
        entries[0].get_str("title"),
        Some("Native LA7 podcast del 23.03.2021")
    );
    assert_eq!(entries[1].get_str("id"), Some("102"));
    assert_eq!(entries[1].get_str("title"), Some("Second episode"));
}
