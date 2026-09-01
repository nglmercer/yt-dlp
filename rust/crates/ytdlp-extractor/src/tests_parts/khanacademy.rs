struct KhanAcademyHandler;

impl RequestHandler for KhanAcademyHandler {
    fn name(&self) -> &str {
        "khanacademy-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if !request
            .url()
            .starts_with("https://www.khanacademy.org/api/internal/graphql/ContentForPath")
        {
            return Err(RequestError::new(
                ErrorKind::Transport,
                format!("no Khan Academy route for {}", request.url()),
            ));
        }
        let body = serde_json::json!({
            "data": {
                "contentRoute": {
                    "listedPathData": {
                        "content": {
                            "id": "716378217",
                            "youtubeId": "FlIG3TvQCBQ",
                            "translatedTitle": "The one-time pad",
                            "thumbnailUrls": [{"url": "https://cdn.example/khan-thumb.jpg", "width": 640}],
                            "duration": 176,
                            "description": "The perfect cipher",
                            "authorNames": ["Brit Cruise"],
                            "dateAdded": "2012-04-11T01:28:33Z",
                            "kaUserLicense": "cc-by-nc-sa"
                        },
                        "course": {
                            "unitChildren": [{
                                "id": "x48c910b6",
                                "relativeUrl": "/computing/computer-science/cryptography",
                                "slug": "cryptography",
                                "translatedTitle": "Cryptography",
                                "translatedDescription": "How have humans protected their secret messages through history?",
                                "allOrderedChildren": [{
                                    "curatedChildren": [{
                                        "contentKind": "Video",
                                        "canonicalUrl": "/computing/computer-science/cryptography/crypt/v/one-time-pad",
                                        "translatedTitle": "The one-time pad"
                                    }]
                                }]
                            }]
                        }
                    }
                }
            }
        });
        Ok(Response::new(
            request.url(),
            200,
            "OK",
            serde_json::to_vec(&body).unwrap(),
        ))
    }
}

fn khanacademy_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KhanAcademyHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn khanacademy_native_video_maps_graphql_metadata() {
    let extractor = KhanAcademyExtractor::new(ExtractorDescriptor::new(
        "KhanAcademyIE",
        "khanacademy",
        r#"https?://(?:www\.)?khanacademy\.org/(?P<id>(?:[^/]+/){4}v/[^?#/&]+)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.khanacademy.org/computing/computer-science/cryptography/crypt/v/one-time-pad",
            &khanacademy_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("_type"), Some("url_transparent"));
    assert_eq!(result.get_str("url"), Some("FlIG3TvQCBQ"));
    assert_eq!(result.get_str("ie_key"), Some("Youtube"));
    assert_eq!(result.get_str("display_id"), Some("716378217"));
    assert_eq!(result.get_str("title"), Some("The one-time pad"));
    assert_eq!(result.get_i64("duration"), Some(176));
    assert_eq!(result.get_str("upload_date"), Some("20120411"));
    assert_eq!(result.get_str("license"), Some("cc-by-nc-sa"));
    assert_eq!(
        result.get("creators"),
        Some(&serde_json::json!(["Brit Cruise"]))
    );
}

#[test]
fn khanacademy_native_unit_builds_video_playlist() {
    let extractor = KhanAcademyUnitExtractor::new(ExtractorDescriptor::new(
        "KhanAcademyUnitIE",
        "khanacademy:unit",
        r#"https?://(?:www\.)?khanacademy\.org/(?P<id>(?:[^/]+/){1,2}[^?#/&]+)/?(?:[?#&]|$)"#,
        true,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context(
            "https://www.khanacademy.org/computing/computer-science/cryptography",
            &khanacademy_context(),
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("display_id"),
        Some("computing/computer-science/cryptography")
    );
    assert_eq!(result.get_str("id"), Some("x48c910b6"));
    assert_eq!(result.get_str("title"), Some("Cryptography"));
    assert_eq!(
        result.get("_old_archive_ids"),
        Some(&serde_json::json!(["khanacademy:unit cryptography"]))
    );
    let entries = result
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].get("url"),
        Some(&serde_json::json!(
            "https://www.khanacademy.org/computing/computer-science/cryptography/crypt/v/one-time-pad"
        ))
    );
    assert_eq!(entries[0].get("ie_key"), Some(&serde_json::json!("KhanAcademy")));
    assert_eq!(
        entries[0].get("title"),
        Some(&serde_json::json!("The one-time pad"))
    );
}
