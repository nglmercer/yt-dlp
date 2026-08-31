/// Determine a URL's extension using the same conservative rules as
/// yt-dlp's utility function. Query strings are excluded, while a trailing
/// slash is accepted for known extension values such as `mp4/`.
pub fn determine_ext(url: Option<&str>, default_ext: &str) -> String {
    let Some(url) = url else {
        return default_ext.to_owned();
    };
    let path = url.split_once('?').map_or(url, |(path, _)| path);
    let Some((_, guess)) = path.rsplit_once('.') else {
        return default_ext.to_owned();
    };
    if !guess.is_empty() && guess.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return guess.to_owned();
    }
    let trimmed = guess.trim_end_matches('/');
    if !trimmed.is_empty()
        && matches!(
            trimmed.to_ascii_lowercase().as_str(),
            "3gp"
                | "aac"
                | "ass"
                | "avi"
                | "flac"
                | "flv"
                | "m4a"
                | "m4v"
                | "mkv"
                | "mov"
                | "m3u8"
                | "mp3"
                | "mp4"
                | "mpeg"
                | "mpg"
                | "oga"
                | "ogg"
                | "opus"
                | "srt"
                | "ssa"
                | "ts"
                | "vtt"
                | "wav"
                | "webm"
                | "webp"
        )
    {
        return trimmed.to_owned();
    }
    default_ext.to_owned()
}

/// Determine the downloader protocol implied by an info dictionary.
pub fn determine_protocol(info: &InfoDict) -> Result<String, CoreError> {
    if let Some(protocol) = info.get_str("protocol") {
        return Ok(protocol.to_owned());
    }
    let url = info.get_str("url").ok_or_else(|| {
        CoreError::new(
            CoreErrorKind::MissingField,
            "determine_protocol requires an info_dict url",
        )
    })?;
    if url.starts_with("rtmp") {
        return Ok("rtmp".to_owned());
    }
    let extension = determine_ext(Some(url), "unknown_video").to_ascii_lowercase();
    if extension == "m3u8" {
        return Ok(if info.get_bool("is_live").unwrap_or(false) {
            "m3u8"
        } else {
            "m3u8_native"
        }
        .to_owned());
    }
    if extension == "f4m" {
        return Ok("f4m".to_owned());
    }
    Ok(URL_SCHEME_RE.find(url).map_or_else(String::new, |scheme| {
        scheme.as_str().trim_end_matches(':').to_owned()
    }))
}

/// Parse an integer-like JSON value with yt-dlp's scaling semantics.
pub fn int_or_none(
    value: Option<&Value>,
    mut scale: i64,
    mut invscale: i64,
    base: Option<u32>,
) -> Option<i64> {
    if invscale == 1 && scale < 1 {
        invscale = (1.0 / scale as f64) as i64;
        scale = 1;
    }
    if scale == 0 {
        return None;
    }
    let integer = match value? {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_f64().map(|value| value as i64)),
        Value::String(value) => base.map_or_else(
            || value.parse::<i64>().ok(),
            |base| i64::from_str_radix(value.trim(), base).ok(),
        ),
        Value::Bool(value) => Some(i64::from(*value)),
        _ => None,
    }?;
    let scaled = integer.checked_mul(invscale)?;
    let quotient = scaled / scale;
    let remainder = scaled % scale;
    if remainder != 0 && ((scaled < 0) != (scale < 0)) {
        quotient.checked_sub(1)
    } else {
        Some(quotient)
    }
}

/// Parse a float-like JSON value with yt-dlp's scaling semantics.
pub fn float_or_none(value: Option<&Value>, scale: f64, invscale: f64) -> Option<f64> {
    if scale == 0.0 {
        return None;
    }
    let value = match value? {
        Value::Number(value) => value.as_f64()?,
        Value::String(value) => value.parse::<f64>().ok()?,
        Value::Bool(value) => f64::from(*value as u8),
        _ => return None,
    };
    let result = value * invscale / scale;
    result.is_finite().then_some(result)
}

/// Convert any JSON-compatible value using Python's string conversion for
/// the common scalar values used by extractor metadata.
pub fn str_or_none(value: Option<&Value>, default: Option<&str>) -> Option<String> {
    let Some(value) = value else {
        return default.map(str::to_owned);
    };
    Some(match value {
        Value::Null => return default.map(str::to_owned),
        Value::String(value) => value.clone(),
        Value::Bool(value) => if *value { "True" } else { "False" }.to_owned(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    })
}
