const FRONTEND_MASTERS_API: &str = "https://api.frontendmasters.com/v1/kabuki";

fn frontendmasters_api(
    context: &ExtractionContext,
    endpoint: &str,
    referer: &str,
) -> Result<serde_json::Value, ExtractorError> {
    native_get_json_with_headers(
        context,
        &format!("{FRONTEND_MASTERS_API}{endpoint}"),
        &[("Referer", referer)],
    )
}

fn frontendmasters_lesson_entry(
    chapters: &[String],
    lesson_id: &str,
    lesson: &serde_json::Value,
) -> InfoDict {
    let title = json_string(lesson, "title")
        .filter(|value| !value.is_empty())
        .unwrap_or(lesson_id);
    let display_id = json_string(lesson, "slug");
    let chapter_number = match (
        json_i64(lesson, "index"),
        json_i64(lesson, "elementIndex"),
    ) {
        (Some(index), Some(element_index)) if index < element_index => {
            Some(element_index - index)
        }
        _ => None,
    };
    let chapter = chapter_number
        .and_then(|number| number.checked_sub(1))
        .and_then(|number| usize::try_from(number).ok())
        .and_then(|index| chapters.get(index))
        .cloned();
    let duration = json_string(lesson, "timestamp").and_then(frontendmasters_duration);
    let mut entry = InfoDict::new();
    entry.insert("_type", serde_json::json!("url_transparent"));
    entry.insert(
        "url",
        serde_json::json!(format!("frontendmasters:{lesson_id}")),
    );
    entry.insert("ie_key", serde_json::json!("FrontendMasters"));
    entry.insert("id", serde_json::json!(lesson_id));
    entry.insert_if_some("display_id", display_id);
    entry.insert("title", serde_json::json!(title));
    entry.insert_if_some("description", json_string(lesson, "description"));
    entry.insert_if_some("thumbnail", json_string(lesson, "thumbnail"));
    entry.insert_if_some("duration", duration);
    entry.insert_if_some("chapter", chapter);
    entry.insert_if_some("chapter_number", chapter_number);
    entry
}

fn frontendmasters_chapters(course: &serde_json::Value) -> Vec<String> {
    course
        .get("lessonElements")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(str::to_owned)
        .collect()
}

fn frontendmasters_duration(value: &str) -> Option<f64> {
    let matcher = Regex::new(
        r#"(?P<start>\d{1,2}:\d{1,2}:\d{1,2})\s*-\s*(?P<end>\d{1,2}:\d{1,2}:\d{1,2})"#,
    )
    .ok()?;
    let captures = matcher.captures(value).ok().flatten()?;
    let start = captures
        .name("start")
        .and_then(|value| yt_dlp_core::parse_duration(value.as_str()))?;
    let end = captures
        .name("end")
        .and_then(|value| yt_dlp_core::parse_duration(value.as_str()))?;
    (end >= start).then_some(end - start)
}

fn frontendmasters_lesson_url(url: &str) -> bool {
    Regex::new(
        r#"^https?://(?:www\.)?frontendmasters\.com/courses/[^/]+/[^/?#]+"#,
    )
    .ok()
    .is_some_and(|matcher| matcher.is_match(url).unwrap_or(false))
}
