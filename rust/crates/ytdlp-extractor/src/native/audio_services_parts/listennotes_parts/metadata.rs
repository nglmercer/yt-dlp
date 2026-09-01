fn listennotes_toolbar_attributes(webpage: &str) -> serde_json::Map<String, serde_json::Value> {
    let Ok(tag_matcher) = Regex::new(r#"(?is)<[^>]+>"#) else {
        return serde_json::Map::new();
    };
    let Ok(attribute_matcher) = Regex::new(
        r#"(?is)\b([A-Za-z_:][A-Za-z0-9_:. -]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#,
    ) else {
        return serde_json::Map::new();
    };
    let mut attributes = serde_json::Map::new();
    for tag in tag_matcher
        .find_iter(webpage)
        .filter_map(|value| value.ok())
        .map(|value| value.as_str())
        .filter(|tag| {
            let tag = tag.to_ascii_lowercase();
            tag.contains("id=\"episode-play-button-toolbar\"")
                || tag.contains("id='episode-play-button-toolbar'")
                || tag.contains("id=\"episode-no-play-button-toolbar\"")
                || tag.contains("id='episode-no-play-button-toolbar'")
        })
    {
        for captures in attribute_matcher.captures_iter(tag).flatten() {
            let Some(name) = captures.get(1).map(|value| value.as_str().to_ascii_lowercase())
            else {
                continue;
            };
            let value = captures
                .get(2)
                .or_else(|| captures.get(3))
                .or_else(|| captures.get(4))
                .map(|value| unescape_html_attribute(value.as_str()))
                .unwrap_or_default();
            attributes.insert(name, serde_json::json!(value));
        }
    }
    attributes
}

fn listennotes_element_by_class(webpage: &str, class_name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<[^>]+\bclass\s*=\s*["'][^"']*\b{}\b[^"']*["'][^>]*>(.*?)</[^>]+>"#,
        regex::escape(class_name)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn listennotes_heading(webpage: &str) -> Option<String> {
    Regex::new(r#"(?is)<h1\b[^>]*>(.*?)</h1>"#)
        .ok()?
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())))
        .filter(|value| !value.trim().is_empty())
}

fn listennotes_description(webpage: &str) -> Option<String> {
    listennotes_element_by_class(webpage, "ln-text-p")
        .map(|value| html_text_fragment(&value))
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let meta = html_meta_value(webpage, "og:description")
                .or_else(|| html_meta_value(webpage, "description"))
                .or_else(|| html_meta_value(webpage, "twitter:description"))?;
            Regex::new(r#"(?s)^[\d:]+\s*-\s*(?P<description>.+)$"#)
                .ok()?
                .captures(meta.trim())
                .ok()
                .flatten()
                .and_then(|captures| captures.name("description"))
                .map(|value| html_text_fragment(value.as_str()))
        })
}

fn listennotes_duration(data: &serde_json::Value) -> Option<f64> {
    json_f64(data, "audio_length")
        .or_else(|| json_f64(data, "data-duration"))
        .or_else(|| {
            json_string(data, "audio_length")
                .or_else(|| json_string(data, "data-duration"))
                .and_then(|value| yt_dlp_core::parse_duration(value))
        })
}

fn listennotes_meta_duration(webpage: &str) -> Option<f64> {
    let meta = html_meta_value(webpage, "og:description")
        .or_else(|| html_meta_value(webpage, "description"))
        .or_else(|| html_meta_value(webpage, "twitter:description"))?;
    Regex::new(r#"(?P<duration>[\d:]+)\s*-"#)
        .ok()?
        .captures(&meta)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("duration"))
        .and_then(|value| yt_dlp_core::parse_duration(value.as_str()))
}
