fn karaoketv_iframe_url(html: &str, marker: &str) -> Option<String> {
    let matcher =
        Regex::new(r#"(?is)<iframe\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#).ok()?;
    matcher
        .captures_iter(html)
        .flatten()
        .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .map(|value| unescape_html_attribute(&value))
        .find(|value| value.contains(marker))
}

fn karaoketv_title(html: &str) -> Option<String> {
    html_meta_value(html, "og:title")
        .or_else(|| html_title_value(html))
        .map(|value| unescape_html_attribute(&value).trim().to_owned())
        .filter(|value| !value.is_empty())
}
