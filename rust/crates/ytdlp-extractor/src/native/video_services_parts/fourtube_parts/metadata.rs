fn fourtube_media_id(html: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<button\b[^>]*\bdata-id\s*=\s*["'](?P<id>\d+)["'][^>]*\bdata-quality\s*="#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
}

fn fourtube_sources(html: &str) -> Vec<String> {
    let Ok(matcher) = Regex::new(
        r#"(?is)<button\b[^>]*\bdata-quality\s*=\s*["']([^"']+)["'][^>]*>"#,
    ) else {
        return Vec::new();
    };
    matcher
        .captures_iter(html)
        .flatten()
        .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .filter(|value| !value.is_empty())
        .collect()
}

fn fourtube_uploader(html: &str) -> (Option<String>, Option<String>) {
    let Ok(anchor_matcher) =
        Regex::new(r#"(?is)<a\b[^>]*\bclass\s*=\s*["']item-to-subscribe["'][^>]*>"#)
    else {
        return (None, None);
    };
    let Ok(href_matcher) = Regex::new(r#"(?is)\bhref\s*=\s*["']([^"']+)["']"#) else {
        return (None, None);
    };
    let Ok(title_matcher) = Regex::new(r#"(?is)\btitle\s*=\s*["']([^"']+)["']"#) else {
        return (None, None);
    };
    anchor_matcher
        .captures_iter(html)
        .flatten()
        .find_map(|captures| {
            let anchor = captures.get(0)?.as_str();
            let href = href_matcher
                .captures(anchor)
                .ok()
                .flatten()
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str())?;
            let path = href.split('?').next()?.trim_end_matches('/');
            let parts = path
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            let owner_index = parts.iter().rposition(|part| {
                matches!(*part, "channel" | "channels" | "user" | "users")
            })?;
            let uploader_id = parts.get(owner_index + 1)?.to_string();
            let uploader = title_matcher
                .captures(anchor)
                .ok()
                .flatten()
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().strip_prefix("Go to "))
                .and_then(|value| value.strip_suffix(" page"))
                .map(str::to_owned)?;
            Some((Some(uploader_id), Some(uploader)))
        })
        .unwrap_or((None, None))
}

fn fourtube_interaction_count(html: &str, interaction: &str) -> Option<i64> {
    let pattern = format!(
        r#"(?is)<meta\b[^>]*\bitemprop\s*=\s*["']interactionCount["'][^>]*\bcontent\s*=\s*["']{}:([0-9,]+)["']"#,
        regex::escape(interaction),
    );
    let matcher = Regex::new(&pattern).ok()?;
    let value = matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().replace(',', ""))?;
    value.parse().ok()
}

fn fourtube_categories(html: &str) -> Option<Vec<String>> {
    let section_matcher = Regex::new(
        r#"(?is)Categories\s*/\s*Tags.*?<ul\b[^>]*\bclass\s*=\s*["'][^"']*\blist\b[^"']*["'][^>]*>(.*?)</ul\s*>"#,
    )
    .ok()?;
    let section = section_matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str())?;
    let item_matcher = Regex::new(r#"(?is)<li\b[^>]*><a\b[^>]*>(.*?)</a\s*>"#).ok()?;
    let categories = item_matcher
        .captures_iter(section)
        .flatten()
        .filter_map(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    (!categories.is_empty()).then_some(categories)
}
