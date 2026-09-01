fn lastfm_youtube_entry(video_url: &str) -> InfoDict {
    let mut entry = native_url_result(video_url);
    entry.insert("ie_key", serde_json::json!("Youtube"));
    entry
}

fn lastfm_page_entries(
    context: &ExtractionContext,
    url: &str,
    playlist_id: &str,
) -> Result<Vec<InfoDict>, ExtractorError> {
    let single_page = url_query_value(url, "page").and_then(|value| value.parse::<i64>().ok());
    let mut page = single_page.unwrap_or(1);
    let matcher = Regex::new(r#"(?i)data-youtube-url\s*=\s*["']([^"']+)["']"#).map_err(
        |error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Last.fm YouTube URL matcher: {error}"),
            )
        },
    )?;
    let mut entries = Vec::new();
    loop {
        let mut request = Request::new(url);
        request.update_query(&[("page".to_owned(), page.to_string())]);
        let response = context.request(&request)?;
        let webpage = String::from_utf8_lossy(response.body());
        let page_entries = matcher
            .captures_iter(&webpage)
            .flatten()
            .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
            .filter(|value| !value.is_empty())
            .map(|value| lastfm_youtube_entry(&value));
        let before = entries.len();
        entries.extend(page_entries);
        if single_page.is_some() || entries.len() == before {
            break;
        }
        page += 1;
    }
    let _ = playlist_id;
    Ok(entries)
}

fn lastfm_playlist_result(
    context: &ExtractionContext,
    url: &str,
    playlist_id: &str,
) -> Result<ExtractorResult, ExtractorError> {
    let entries = lastfm_page_entries(context, url, playlist_id)?;
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(playlist_id));
    Ok(ExtractorResult::Playlist { info, entries })
}
