#[derive(Debug, Clone)]
struct LifeNewsMetadata {
    title: String,
    description: String,
    view_count: Option<i64>,
    timestamp: Option<i64>,
}

fn lifenews_page_media(page_url: &str, webpage: &str) -> (Vec<String>, Vec<String>) {
    let video_urls = Regex::new(
        r#"(?is)<video\b[^>]*>\s*<source\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#,
    )
    .ok()
    .map(|matcher| {
        matcher
            .captures_iter(webpage)
            .flatten()
            .filter_map(|captures| captures.get(1))
            .map(|value| resolve_url(page_url, &unescape_html_attribute(value.as_str())))
            .collect()
    })
    .unwrap_or_default();
    let iframe_links = Regex::new(
        r#"(?is)<iframe\b[^>]*\bsrc\s*=\s*["']((?:https?:)?//embed\.life\.ru/(?:embed|video)/[^"']+)"#,
    )
    .ok()
    .map(|matcher| {
        matcher
            .captures_iter(webpage)
            .flatten()
            .filter_map(|captures| captures.get(1))
            .map(|value| {
                proto_relative_url(
                    &unescape_html_attribute(value.as_str()),
                    "http:",
                )
            })
            .collect()
    })
    .unwrap_or_default();
    (video_urls, iframe_links)
}

fn lifenews_metadata(
    webpage: &str,
    video_id: &str,
) -> Result<LifeNewsMetadata, ExtractorError> {
    let title = html_meta_value(webpage, "og:title")
        .map(|value| html_text_fragment(&value))
        .map(|value| {
            value
                .strip_suffix(" - Life.ru")
                .unwrap_or(&value)
                .to_owned()
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Life.ru article {video_id} has no Open Graph title"),
            )
        })?;
    let description = html_meta_value(webpage, "og:description")
        .map(|value| html_text_fragment(&value))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Life.ru article {video_id} has no Open Graph description"),
            )
        })?;
    let view_count = Regex::new(
        r#"(?is)<div\b[^>]*\bclass\s*=\s*(["'])[^"']*\bhits-count\b[^"']*\1[^>]*>\s*(\d+)\s*</div>"#,
    )
    .ok()
    .and_then(|matcher| matcher.captures(webpage).ok().flatten())
    .and_then(|captures| captures.get(2))
    .and_then(|value| value.as_str().parse::<i64>().ok());
    let timestamp = Regex::new(
        r#"(?is)<time\b[^>]*\bdatetime\s*=\s*(["'])([^"']+)\1"#,
    )
    .ok()
    .and_then(|matcher| matcher.captures(webpage).ok().flatten())
    .and_then(|captures| captures.get(2))
    .and_then(|value| parse_timestamp(value.as_str().to_owned()));
    Ok(LifeNewsMetadata {
        title,
        description,
        view_count,
        timestamp,
    })
}
