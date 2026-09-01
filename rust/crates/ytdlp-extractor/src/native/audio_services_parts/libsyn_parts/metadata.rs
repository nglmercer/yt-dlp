fn libsyn_element_by_class(html: &str, class_name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<[^>]+\bclass\s*=\s*["'][^"']*\b{}\b[^"']*["'][^>]*>(.*?)</[^>]+>"#,
        regex::escape(class_name)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())))
        .filter(|value| !value.trim().is_empty())
}

fn libsyn_title_tag(html: &str) -> Option<String> {
    Regex::new(r#"(?is)<title\b[^>]*>(.*?)</title>"#)
        .ok()?
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())))
        .filter(|value| !value.trim().is_empty())
}

fn libsyn_episode_title(data: &serde_json::Value, webpage: &str) -> Option<String> {
    json_string(data, "item_title")
        .map(str::to_owned)
        .or_else(|| libsyn_element_by_class(webpage, "episode-title"))
        .or_else(|| {
            Regex::new(r#"(?is)\bdata-title\s*=\s*["']([^"']+)["']"#)
                .ok()?
                .captures(webpage)
                .ok()
                .flatten()
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| unescape_html_attribute(value.as_str()))
                })
        })
        .or_else(|| libsyn_title_tag(webpage))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn libsyn_podcast_title(webpage: &str) -> Option<String> {
    Regex::new(r#"(?is)<h3\b[^>]*>([^<]+)</h3>"#)
        .ok()?
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())))
        .or_else(|| libsyn_element_by_class(webpage, "podcast-title"))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn libsyn_description(webpage: &str) -> Option<String> {
    html_element_by_id(webpage, "info_text_body")
        .map(|value| html_text_fragment(&value).replace('\u{a0}', " "))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn libsyn_release_date(data: &serde_json::Value, webpage: &str) -> Option<String> {
    let raw = Regex::new(
        r#"(?is)<div\b[^>]*class\s*=\s*["']release_date["'][^>]*>\s*Released:\s*([^<]+)<"#,
    )
    .ok()
    .and_then(|matcher| matcher.captures(webpage).ok().flatten())
    .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
    .or_else(|| json_string(data, "release_date").map(str::to_owned))?;
    date_digits(raw.trim())
}
