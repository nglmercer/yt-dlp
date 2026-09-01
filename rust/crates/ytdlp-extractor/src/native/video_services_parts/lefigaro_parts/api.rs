const LEFIGARO_GRAPHQL_QUERY_ID: &str =
    "flive-website_UpdateListPage_1fb260f996bca2d78960805ac382544186b3225f5bedb43ad08b9b8abef79af6";
const LEFIGARO_PAGE_SIZE: i64 = 20;

fn lefigaro_api_response(
    context: &ExtractionContext,
    display_id: &str,
    page: i64,
) -> Result<serde_json::Value, ExtractorError> {
    let variables = serde_json::json!({
        "slug": display_id,
        "videosLimit": LEFIGARO_PAGE_SIZE,
        "sort": "DESC",
        "order": "PUBLISHED_AT",
        "page": page,
    });
    let mut request = Request::new("https://api-graphql.lefigaro.fr/graphql");
    request.update_query(&[
        (
            "id".to_owned(),
            LEFIGARO_GRAPHQL_QUERY_ID.to_owned(),
        ),
        (
            "variables".to_owned(),
            serde_json::to_string(&variables).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("could not encode Le Figaro GraphQL variables: {error}"),
                )
            })?,
        ),
    ]);
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Le Figaro GraphQL JSON for {display_id}: {error}"),
        )
    })
}

fn lefigaro_playlist<'a>(
    response: &'a serde_json::Value,
    display_id: &str,
) -> Result<&'a serde_json::Value, ExtractorError> {
    response
        .get("data")
        .and_then(|data| data.get("playlist"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Le Figaro section {display_id} has no playlist object"),
            )
        })
}

fn lefigaro_page_count(response: &serde_json::Value, display_id: &str) -> Result<i64, ExtractorError> {
    let video_count = json_i64(
        lefigaro_playlist(response, display_id)?,
        "videoCount",
    )
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Le Figaro section {display_id} has no video count"),
        )
    })?;
    Ok(if video_count <= 0 {
        0
    } else {
        (video_count + LEFIGARO_PAGE_SIZE - 1) / LEFIGARO_PAGE_SIZE
    })
}
