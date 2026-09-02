fn youtube_json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| youtube_text(value))
    })
}

fn youtube_text(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_owned());
    }
    if let Some(text) = value.get("simpleText").and_then(serde_json::Value::as_str) {
        return Some(text.to_owned());
    }
    let runs = value.get("runs")?.as_array()?;
    let text = runs
        .iter()
        .filter_map(|run| run.get("text").and_then(serde_json::Value::as_str))
        .collect::<String>();
    (!text.is_empty()).then_some(text)
}

fn youtube_json_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn youtube_json_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn youtube_json_bool(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(serde_json::Value::as_bool)
}

fn youtube_player_responses_from_page(
    webpage: &str,
    video_id: &str,
) -> Vec<serde_json::Value> {
    json_objects_after_marker(webpage, "ytInitialPlayerResponse")
        .into_iter()
        .filter(|response| {
            response
                .get("videoDetails")
                .and_then(|details| youtube_json_string(details, "videoId"))
                .is_none_or(|id| id == video_id)
        })
        .collect()
}

fn youtube_select_player_response(
    responses: &[serde_json::Value],
    video_id: &str,
) -> Option<serde_json::Value> {
    responses
        .iter()
        .find(|response| {
            response
                .get("videoDetails")
                .and_then(|details| youtube_json_string(details, "videoId"))
                .as_deref()
                == Some(video_id)
                && youtube_response_has_streaming_data(response)
        })
        .or_else(|| {
            responses.iter().find(|response| {
                response
                    .get("videoDetails")
                    .and_then(|details| youtube_json_string(details, "videoId"))
                    .as_deref()
                    == Some(video_id)
            })
        })
        .cloned()
}

fn youtube_response_has_streaming_data(response: &serde_json::Value) -> bool {
    let Some(streaming) = response.get("streamingData") else {
        return false;
    };
    streaming.get("formats").and_then(serde_json::Value::as_array).is_some_and(|formats| !formats.is_empty())
        || streaming
            .get("adaptiveFormats")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|formats| !formats.is_empty())
        || streaming.get("hlsManifestUrl").is_some()
        || streaming.get("dashManifestUrl").is_some()
}
