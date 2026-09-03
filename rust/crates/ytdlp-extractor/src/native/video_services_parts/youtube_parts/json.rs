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

const YOUTUBE_LOGIN_HINT: &str = "Login with cookies is required to access this content. Also see  https://github.com/yt-dlp/yt-dlp/wiki/Extractors#exporting-youtube-cookies  for tips on effectively exporting YouTube cookies";

/// Mirrors the no-formats branch of `_real_extract`: DRM-gated videos fail
/// fast, otherwise the player error-screen reason and subreason are composed
/// with the sign-in, captcha, and rate-limit rewrites.
pub(crate) fn youtube_no_formats_error(
    video_id: &str,
    responses: &[serde_json::Value],
    todos: &[String],
) -> ExtractorError {
    let has_license = responses.iter().any(|response| {
        response
            .get("streamingData")
            .and_then(|streaming| streaming.get("licenseInfos"))
            .is_some()
    });
    if has_license {
        return ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            "This video is DRM protected",
        );
    }
    let statuses = responses
        .iter()
        .filter_map(|response| response.get("playabilityStatus"))
        .collect::<Vec<_>>();
    let pemr = statuses
        .iter()
        .find_map(|status| status.get("errorScreen")?.get("playerErrorMessageRenderer"))
        .unwrap_or(&serde_json::Value::Null);
    let mut reason = pemr.get("reason").and_then(youtube_text).or_else(|| {
        statuses.iter().find_map(|status| {
            status.get("reason").and_then(|reason| {
                reason
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| youtube_text(reason))
            })
        })
    });
    let subreason = pemr
        .get("subreason")
        .and_then(youtube_text)
        .map(|subreason| html_text_fragment(&subreason))
        .filter(|subreason| !subreason.is_empty());
    if let Some(subreason) = subreason {
        // Geo-blocked videos keep their region messaging; without a dedicated
        // geo error kind the composed message carries the same text.
        let composed = match reason {
            Some(reason) => format!("{reason}. {subreason}"),
            None => subreason,
        };
        reason = Some(composed);
    }
    if let Some(text) = reason {
        let mut text = text;
        if text.to_ascii_lowercase().contains("sign in") {
            text = text.replace("This helps protect our community. Learn more", "");
            let trimmed = text.trim().trim_end_matches('.').to_owned();
            text = format!("{trimmed}. {YOUTUBE_LOGIN_HINT}");
        } else if statuses.iter().any(|status| {
            status
                .get("errorScreen")
                .and_then(|screen| screen.get("playerCaptchaViewModel"))
                .is_some()
        }) {
            text = format!("{text}. YouTube is requiring a captcha challenge before playback");
        } else if text.contains("This content isn't available, try again later") {
            let trimmed = text.trim().trim_end_matches('.').to_owned();
            text = format!(
                "{trimmed}. The current session has been rate-limited by YouTube for up to an hour. It is recommended to add a delay between video requests to avoid exceeding the rate limit"
            );
        }
        if todos.is_empty() {
            return ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("YouTube video {video_id}: {text}"),
            );
        }
    } else if todos.is_empty() {
        return ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("YouTube video {video_id}: YouTube returned no downloadable formats"),
        );
    }
    ExtractorError::new(ExtractorErrorKind::Unsupported, todos.join("; "))
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
