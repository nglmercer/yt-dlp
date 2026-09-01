const LOOM_GRAPHQL_VERSION: &str = "45a5bd4";

fn loom_graphql_query(operation: &str) -> &'static str {
    match operation {
        "GetVideoSSR" => {
            r#"query GetVideoSSR($videoId: ID!, $password: String) {
                getVideo(id: $videoId, password: $password) {
                    __typename
                    ... on PrivateVideo { id status message __typename }
                    ... on VideoPasswordMissingOrIncorrect { id message __typename }
                    ... on RegularUserVideo {
                        id createdAt description name
                        owner { display_name __typename }
                        video_properties {
                            duration width height microphone_enabled
                            __typename
                        }
                        __typename
                    }
                }
            }"#
        }
        "GetVideoSource" => {
            r#"query GetVideoSource($videoId: ID!, $password: String, $acceptableMimes: [CloudfrontVideoAcceptableMime]) {
                getVideo(id: $videoId, password: $password) {
                    ... on RegularUserVideo {
                        id
                        nullableRawCdnUrl(acceptableMimes: $acceptableMimes, password: $password) {
                            url
                            credentials { Policy Signature KeyPairId __typename }
                            __typename
                        }
                        __typename
                    }
                    __typename
                }
            }"#
        }
        "FetchVideoTranscript" => {
            r#"query FetchVideoTranscript($videoId: ID!, $password: String) {
                fetchVideoTranscript(videoId: $videoId, password: $password) {
                    ... on VideoTranscriptDetails {
                        id video_id source_url captions_source_url __typename
                    }
                    ... on GenericError { message __typename }
                    __typename
                }
            }"#
        }
        "FetchChapters" => {
            r#"query FetchChapters($videoId: ID!, $password: String) {
                fetchVideoChapters(videoId: $videoId, password: $password) {
                    ... on VideoChapters { video_id content __typename }
                    ... on EmptyChaptersPayload { content __typename }
                    ... on InvalidRequestWarning { message __typename }
                    ... on Error { message __typename }
                    __typename
                }
            }"#
        }
        _ => "",
    }
}

fn loom_graphql(
    context: &ExtractionContext,
    operation: &str,
    video_id: &str,
    optional: bool,
) -> Result<serde_json::Value, ExtractorError> {
    let mut variables = serde_json::json!({
        "videoId": video_id,
        "password": serde_json::Value::Null,
    });
    if operation == "GetVideoSource" {
        variables["acceptableMimes"] = serde_json::json!(["DASH", "M3U8", "MP4", "WEBM"]);
    }
    let payload = serde_json::json!({
        "operationName": operation,
        "variables": variables,
        "query": loom_graphql_query(operation),
    });
    let mut request = Request::new("https://www.loom.com/graphql");
    request.set_method("POST").map_err(map_request_error)?;
    request.headers_mut().set("Accept", "application/json");
    request.headers_mut().set("Content-Type", "application/json");
    request
        .headers_mut()
        .set("x-loom-request-source", &format!("loom_web_{LOOM_GRAPHQL_VERSION}"));
    request
        .headers_mut()
        .set("apollographql-client-name", "web");
    request
        .headers_mut()
        .set("apollographql-client-version", LOOM_GRAPHQL_VERSION);
    request
        .headers_mut()
        .set("graphql-operation-name", operation);
    request.headers_mut().set("Origin", "https://www.loom.com");
    request.set_data(Some(serde_json::to_vec(&payload).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("could not encode Loom {operation} request: {error}"),
        )
    })?));
    let response = if optional {
        match context.request(&request) {
            Ok(response) => response,
            Err(error) => return Err(error),
        }
    } else {
        context.request(&request)?
    };
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Loom {operation} JSON for {video_id}: {error}"),
        )
    })
}

fn loom_url_api(
    context: &ExtractionContext,
    video_id: &str,
    endpoint: &str,
) -> Result<Option<String>, ExtractorError> {
    let payload = serde_json::json!({
        "anonID": format!("native-rust-{video_id}"),
        "deviceID": serde_json::Value::Null,
        "force_original": false,
        "password": serde_json::Value::Null,
    });
    let mut request = Request::new(format!(
        "https://www.loom.com/api/campaigns/sessions/{video_id}/{endpoint}"
    ));
    request.set_method("POST").map_err(map_request_error)?;
    request.headers_mut().set("Accept", "application/json");
    request.headers_mut().set("Content-Type", "application/json");
    request.set_data(Some(serde_json::to_vec(&payload).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("could not encode Loom {endpoint} request: {error}"),
        )
    })?));
    let response = match context.request(&request) {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    let data: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Loom {endpoint} JSON for {video_id}: {error}"),
        )
    })?;
    Ok(json_string(&data, "url")
        .filter(|value| !value.is_empty())
        .map(str::to_owned))
}
