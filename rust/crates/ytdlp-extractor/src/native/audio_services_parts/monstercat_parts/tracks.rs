fn monstercat_attribute(tag: &str, attribute: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)\b{}\s*=\s*[\"']([^\"']*)"#,
        regex::escape(attribute)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(tag)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
}

fn monstercat_track_rows(table: &str, album_meta: &InfoDict) -> Vec<InfoDict> {
    let Ok(row_matcher) = Regex::new(r"(?is)<tr\b[^>]*>(.*?)</tr\s*>") else {
        return Vec::new();
    };
    let Ok(play_button_matcher) = Regex::new(
        r#"(?is)<[a-z0-9]+\b[^>]*\bclass\s*=\s*[\"'][^\"']*\bbtn-play\b[^\"']*\bcursor-pointer\b[^\"']*\bmr-small\b[^\"']*[\"'][^>]*>"#,
    ) else {
        return Vec::new();
    };
    let Ok(title_matcher) = Regex::new(
        r#"(?is)<(?P<tag>[a-z0-9]+)\b[^>]*\bclass\s*=\s*[\"'][^\"']*\bd-inline-flex\b[^\"']*\bflex-column\b[^\"']*[\"'][^>]*>(.*?)</(?P=tag)\s*>"#,
    ) else {
        return Vec::new();
    };
    let Ok(track_number_matcher) = Regex::new(
        r#"(?is)<(?P<tag>[a-z0-9]+)\b[^>]*\bclass\s*=\s*[\"'][^\"']*\bpy-xsmall\b[^\"']*[\"'][^>]*>(.*?)</(?P=tag)\s*>"#,
    ) else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    for row in row_matcher.captures_iter(table).flatten() {
        let Some(row_html) = row.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(play_button) = play_button_matcher
            .find(row_html)
            .ok()
            .flatten()
            .map(|value| value.as_str())
        else {
            continue;
        };
        let Some(track_id) = monstercat_attribute(play_button, "data-track-id") else {
            continue;
        };
        let Some(release_id) = monstercat_attribute(play_button, "data-release-id") else {
            continue;
        };
        let title = title_matcher
            .captures(row_html)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(2))
            .map(|value| {
                let value = value
                    .as_str()
                    .split_once(" <span")
                    .map_or(value.as_str(), |(value, _)| value);
                html_text_fragment(value)
            })
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let track_number = track_number_matcher
            .captures(row_html)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(2))
            .and_then(|value| html_text_fragment(value.as_str()).trim().parse::<i64>().ok());
        let artists = monstercat_class_values(row_html, "d-block fs-xxsmall")
            .into_iter()
            .map(|value| html_text_fragment(&value))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

        let mut entry = album_meta.clone();
        let media_url = format!(
            "https://www.monstercat.com/api/release/{release_id}/track-stream/{track_id}"
        );
        entry.insert("id", serde_json::json!(track_id));
        entry.insert("url", serde_json::json!(media_url.clone()));
        entry.insert("ext", serde_json::json!("mp3"));
        entry.insert("vcodec", serde_json::json!("none"));
        entry.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "mp3",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        entry.insert_if_some("title", title.clone());
        entry.insert_if_some("track", title);
        entry.insert_if_some("track_number", track_number);
        entry.insert_if_some("artists", (!artists.is_empty()).then_some(artists));
        entries.push(entry);
    }
    entries
}
