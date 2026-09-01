fn lecturio_language_code(value: &str) -> String {
    let code = match value {
        "Arabic" => "ar",
        "Bulgarian" => "bg",
        "German" => "de",
        "English" => "en",
        "Spanish" => "es",
        "Persian" => "fa",
        "French" => "fr",
        "Japanese" => "ja",
        "Polish" => "pl",
        "Pashto" => "ps",
        "Russian" => "ru",
        value => value,
    };
    code.to_owned()
}

fn lecturio_file_size(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
    })?;
    value.checked_mul(1_000)
}

fn lecturio_formats(
    video: &serde_json::Value,
    lecture_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let media = video
        .get("content")
        .and_then(|content| content.get("media"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Lecturio lecture {lecture_id} has no media list"),
            )
        })?;
    let label_matcher = Regex::new(r#"^(\d+)p\s*\(([^)]+)\)"#).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Lecturio format label matcher: {error}"),
        )
    })?;
    let mut formats = Vec::new();
    for media_item in media {
        let Some(file_url) = json_string(media_item, "file").filter(|value| !value.is_empty())
        else {
            continue;
        };
        let extension = yt_dlp_core::determine_ext(Some(file_url), "mp4");
        if extension.eq_ignore_ascii_case("smil") {
            continue;
        }
        let label = json_string(media_item, "label").map(str::to_owned);
        let mut format = serde_json::json!({"url": file_url});
        if let Some(filesize) = lecturio_file_size(media_item.get("fileSize")) {
            format["filesize"] = serde_json::json!(filesize);
        }
        if let Some(label) = label {
            format["format_id"] = serde_json::json!(label);
            if let Some(captures) = label_matcher.captures(&label).ok().flatten() {
                if let Some(height) = captures.get(1).and_then(|value| value.as_str().parse::<i64>().ok()) {
                    format["height"] = serde_json::json!(height);
                }
                if let Some(format_id) = captures.get(2).map(|value| value.as_str()) {
                    format["format_id"] = serde_json::json!(format_id);
                }
            }
        }
        format["ext"] = serde_json::json!(extension);
        formats.push(format);
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Lecturio lecture {lecture_id} has no playable media"),
        ));
    }
    Ok(formats)
}

fn lecturio_caption_language(url: &str, label: Option<&str>) -> String {
    let language = Regex::new(r#"/([a-z]{2})_"#)
        .ok()
        .and_then(|matcher| matcher.captures(url).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .or_else(|| label.and_then(|value| value.split_whitespace().next()).map(str::to_owned))
        .unwrap_or_else(|| "en".to_owned());
    lecturio_language_code(&language)
}

fn lecturio_subtitles(video: &serde_json::Value) -> (serde_json::Value, serde_json::Value) {
    let mut subtitles = serde_json::Map::new();
    let mut automatic_captions = serde_json::Map::new();
    let Some(captions) = video.get("captions").and_then(serde_json::Value::as_array) else {
        return (
            serde_json::Value::Object(subtitles),
            serde_json::Value::Object(automatic_captions),
        );
    };
    let original_language_matcher = Regex::new(r#"/[a-z]{2}_([a-z]{2})_"#).ok();
    for caption in captions {
        let Some(url) = json_string(caption, "url").filter(|value| !value.is_empty()) else {
            continue;
        };
        let label = json_string(caption, "translatedCode");
        let language = json_string(caption, "languageCode")
            .map(lecturio_language_code)
            .unwrap_or_else(|| lecturio_caption_language(url, label));
        let is_automatic = label.is_some_and(|value| value.contains("auto-translated"))
            || original_language_matcher
                .as_ref()
                .and_then(|matcher| matcher.captures(url).ok().flatten())
                .is_some();
        let destination = if is_automatic {
            &mut automatic_captions
        } else {
            &mut subtitles
        };
        destination
            .entry(language)
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .expect("caption map values are arrays")
            .push(serde_json::json!({"url": url}));
    }
    (
        serde_json::Value::Object(subtitles),
        serde_json::Value::Object(automatic_captions),
    )
}

fn lecturio_video_info(
    video: &serde_json::Value,
    lecture_id: &str,
) -> Result<InfoDict, ExtractorError> {
    let title = json_string(video, "title")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Lecturio lecture {lecture_id} has no title"),
            )
        })?;
    let formats = lecturio_formats(video, lecture_id)?;
    let (subtitles, automatic_captions) = lecturio_subtitles(video);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(lecture_id));
    info.insert("title", serde_json::json!(title));
    info.insert("formats", serde_json::Value::Array(formats));
    info.insert("subtitles", subtitles);
    info.insert("automatic_captions", automatic_captions);
    Ok(info)
}
