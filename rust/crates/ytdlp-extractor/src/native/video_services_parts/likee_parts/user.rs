fn likee_user_id_value(user_info: &serde_json::Value) -> Option<serde_json::Value> {
    user_info.get("uid").cloned().filter(|value| !value.is_null())
}

fn likee_value_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .or_else(|| value.as_u64().map(|value| value.to_string()))
}

fn likee_user_entries(
    context: &ExtractionContext,
    user_name: &str,
    user_id: &serde_json::Value,
) -> Result<Vec<InfoDict>, ExtractorError> {
    let endpoint = "https://api.like-video.com/likee-activity-flow-micro/videoApi/getUserVideo";
    let mut last_post_id = String::new();
    let mut entries = Vec::new();
    loop {
        let payload = serde_json::json!({
            "uid": user_id,
            "count": 50,
            "lastPostId": last_post_id,
            "tabType": 0,
        });
        let mut request = Request::new(endpoint);
        request.set_method("POST").map_err(map_request_error)?;
        request.headers_mut().set("content-type", "application/json");
        request.set_data(Some(serde_json::to_vec(&payload).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("could not encode Likee user request: {error}"),
            )
        })?));
        let response = context.request(&request)?;
        let response_data: serde_json::Value = serde_json::from_slice(response.body()).map_err(
            |error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Likee user JSON: {error}"),
                )
            },
        )?;
        let Some(items) = response_data
            .get("data")
            .and_then(|data| data.get("videoList"))
            .and_then(serde_json::Value::as_array)
        else {
            break;
        };
        if items.is_empty() {
            break;
        }
        let previous_last_post_id = last_post_id.clone();
        for item in items {
            let Some(post_id) = item.get("postId").and_then(likee_value_string) else {
                continue;
            };
            last_post_id = post_id.clone();
            entries.push(native_url_result(&format!(
                "https://likee.video/{user_name}/video/{post_id}"
            )));
        }
        if last_post_id.is_empty() || last_post_id == previous_last_post_id {
            break;
        }
    }
    Ok(entries)
}
