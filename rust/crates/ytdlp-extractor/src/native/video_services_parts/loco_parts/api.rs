const LOCO_CLIENT_ID: &str = "TlwKp1zmF6eKFpcisn3FyR18WkhcPkZtzwPVEEC3";
const LOCO_CLIENT_SECRET: &str = "Kp7tYlUN7LXvtcSpwYvIitgYcLparbtsQSe5AdyyCdiEJBP53Vt9J8eB4AsLdChIpcO2BM19RA3HsGtqDJFjWmwoonvMSG3ZQmnS8x1YIM8yl82xMXZGbE3NKiqmgBVU";

fn loco_find_stream(value: &serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::Object(values) => {
            if values
                .get("uid")
                .and_then(serde_json::Value::as_str)
                .is_some()
                && values
                    .get("conf")
                    .and_then(|conf| json_string(conf, "hls"))
                    .is_some()
            {
                return Some(value.clone());
            }
            values.values().find_map(loco_find_stream)
        }
        serde_json::Value::Array(values) => values.iter().find_map(loco_find_stream),
        _ => None,
    }
}

fn loco_page_stream(webpage: &str, video_id: &str) -> Result<serde_json::Value, ExtractorError> {
    let next_data = html_script_json(webpage, "__NEXT_DATA__")
        .ok()
        .or_else(|| json_object_after_marker(webpage, "liveStreamData"));
    next_data
        .and_then(|data| loco_find_stream(&data))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Loco page {video_id} has no native stream object"),
            )
        })
}

fn loco_access_token(context: &ExtractionContext, video_id: &str) -> Option<String> {
    let payload = serde_json::json!({
        "platform": 7,
        "client_id": LOCO_CLIENT_ID,
        "client_secret": LOCO_CLIENT_SECRET,
        "model": "Mozilla",
        "os_name": "Win32",
        "os_ver": "5.0 (Windows)",
        "app_ver": "5.0 (Windows)",
    });
    let mut request = Request::new("https://api.getloconow.com/v3/user/device_profile/");
    request.set_method("POST").ok()?;
    request.headers_mut().set("Content-Type", "application/json;charset=utf-8");
    request.headers_mut().set("DEVICE-ID", format!("native-rust-{video_id}"));
    request.headers_mut().set("X-APP-LANG", "en");
    request.headers_mut().set("X-APP-LOCALE", "en-US");
    request.headers_mut().set("X-CLIENT-ID", LOCO_CLIENT_ID);
    request.headers_mut().set("X-CLIENT-SECRET", LOCO_CLIENT_SECRET);
    request.headers_mut().set("X-PLATFORM", "7");
    request
        .set_data(Some(serde_json::to_vec(&payload).ok()?));
    let response = context.request(&request).ok()?;
    let data: serde_json::Value = serde_json::from_slice(response.body()).ok()?;
    json_string(&data, "access_token").map(str::to_owned)
}

fn loco_authorize(
    context: &ExtractionContext,
    video_id: &str,
    stream_uid: &str,
    access_token: &str,
) {
    let payload = serde_json::json!({"stream_uid": stream_uid});
    let mut request = Request::new("https://drm.loco.com/v1/streams/playback/");
    if request.set_method("POST").is_err() {
        return;
    }
    request.headers_mut().set("Content-Type", "application/json");
    request.headers_mut().set("authorization", access_token);
    request.set_data(serde_json::to_vec(&payload).ok());
    let _ = context.request(&request);
    let _ = video_id;
}
