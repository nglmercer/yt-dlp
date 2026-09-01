fn kankanews_page_field(html: &str, field: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is){}\s*=\s*\"([^\"]*)\""#,
        regex::escape(field)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn kankanews_video_id(html: &str) -> Option<String> {
    kankanews_page_field(html, "omsid").filter(|value| value.chars().all(|c| c.is_ascii_digit()))
}

fn kankanews_title(html: &str) -> Option<String> {
    kankanews_page_field(html, "g.title").filter(|value| !value.is_empty())
}
