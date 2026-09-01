fn mx3_class_fragment(html: &str, class_name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<(?P<tag>[a-z0-9]+)\b[^>]*\bclass\s*=\s*[\"'][^\"']*\b{}\b[^\"']*[\"'][^>]*>(.*?)</(?P=tag)\s*>"#,
        regex::escape(class_name)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(2))
        .map(|value| value.as_str().to_owned())
}

fn mx3_info_field(more_info: &str, field: &str) -> Option<String> {
    let pattern = format!(
        r"(?is)<dt[^>]*>\s*{}\s*</dt>\s*<dd[^>]*>(.*?)</dd\s*>",
        regex::escape(field)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(more_info)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn mx3_genre(html: &str) -> Option<String> {
    let fragment = mx3_class_fragment(html, "single-band-genre")?;
    let genre = html_text_fragment(&fragment);
    (!genre.is_empty()).then_some(genre)
}

fn mx3_tags(more_info: &str) -> Option<Vec<String>> {
    let value = mx3_info_field(more_info, "Tag")?;
    Some(
        value
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}
