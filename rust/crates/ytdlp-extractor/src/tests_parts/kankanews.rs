struct KankaNewsHandler;

impl RequestHandler for KankaNewsHandler {
    fn name(&self) -> &str {
        "kankanews-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("www.kankanews.com/a/") {
            let body = br#"
                <script>
                    var omsid = "4485057";
                    g.title = "Native KankaNews title";
                </script>
            "#;
            return Ok(Response::new(url, 200, "OK", body.to_vec()));
        }
        if url.starts_with("https://api-app.kankanews.com/kankan/pc/getvideo") {
            let parsed = url::Url::parse(url).expect("test API URL is valid");
            let query = parsed
                .query_pairs()
                .collect::<std::collections::HashMap<_, _>>();
            assert_eq!(query.get("omsid").map(|value| value.as_ref()), Some("4485057"));
            assert_eq!(query.get("platform").map(|value| value.as_ref()), Some("pc"));
            assert_eq!(query.get("version").map(|value| value.as_ref()), Some("1.0"));
            assert_eq!(query.get("nonce").map(|value| value.len()), Some(8));
            assert_eq!(query.get("sign").map(|value| value.len()), Some(32));
            let body = br#"{
                "result": {
                    "video": {
                        "videourl": "https://cdn.example/kankanews.mp4",
                        "titlepic": "https://cdn.example/kankanews.jpg"
                    }
                }
            }"#;
            return Ok(Response::new(url, 200, "OK", body.to_vec()));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no KankaNews route for {url}"),
        ))
    }
}

fn kankanews_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KankaNewsHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kankanews_native_extractor_signs_api_and_maps_media() {
    let extractor = KankaNewsExtractor::new(ExtractorDescriptor::new(
        "KankaNewsIE",
        "KankaNews",
        r#"https?://(?:www\.)?kankanews\.com/a/\d+\-\d+\-\d+/(?P<id>\d+)\.shtml"#,
        false,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.kankanews.com/a/2022-11-08/00310276054.shtml?appid=1088227",
            &kankanews_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("4485057"));
    assert_eq!(result.get_str("title"), Some("Native KankaNews title"));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/kankanews.mp4")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/kankanews.jpg")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
}
