struct ListenNotesHandler;

impl RequestHandler for ListenNotesHandler {
    fn name(&self) -> &str {
        "listen-notes-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if !request
            .url()
            .contains("listennotes.com/podcasts/native-show/native-episode-NativeAudio42")
        {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Listen Notes route for {}", request.url()),
            ));
        }
        let webpage = r#"
            <meta property="og:description" content="2215 - Fallback description">
            <script id="original-content" type="application/json">
                {
                    "audio_length": "36:55",
                    "uuid": "native-episode-uuid",
                    "nlp_entities": [{"name": "Native guest"}]
                }
            </script>
            <div id="episode-play-button-toolbar"
                audio="https://cdn.example/listennotes/native.mp3"
                data-title="Native Listen Notes episode"
                data-image="https://cdn.example/listennotes/native.jpg"
                data-channel-title="Native channel"
                channel_url="https://www.listennotes.com/podcasts/native-show/"
                channel_short_uuid="native-channel-id"
                data-duration="2215"
                data-episode-uuid="native-data-uuid">
            </div>
            <div class="ln-text-p">Native episode description</div>
        "#;
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            webpage.as_bytes().to_vec(),
        ))
    }
}

fn listen_notes_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(ListenNotesHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn listen_notes_native_extractor_maps_audio_attributes_and_episode_metadata() {
    let extractor = ListenNotesExtractor::new(ExtractorDescriptor::new(
        "ListenNotesIE",
        "ListenNotes",
        r#"https?://(?:www\.)?listennotes\.com/podcasts/[^/]+/[^/]+-(?P<id>.+)/"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.listennotes.com/podcasts/native-show/native-episode-NativeAudio42/",
            &listen_notes_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(result.get_str("id"), Some("NativeAudio42"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/listennotes/native.mp3")
    );
    assert_eq!(
        result.get_str("title"),
        Some("Native Listen Notes episode")
    );
    assert_eq!(
        result.get_str("description"),
        Some("Native episode description")
    );
    assert_eq!(result.get_f64("duration"), Some(2215.0));
    assert_eq!(result.get_str("episode_id"), Some("native-episode-uuid"));
    assert_eq!(result.get_str("channel"), Some("Native channel"));
    assert_eq!(result.get_str("channel_id"), Some("native-channel-id"));
    assert_eq!(
        result.get("cast"),
        Some(&serde_json::json!(["Native guest"]))
    );
}
