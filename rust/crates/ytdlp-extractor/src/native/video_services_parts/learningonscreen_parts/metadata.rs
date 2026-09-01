fn los_first_match(html: &str, pattern: &str) -> Option<String> {
    let matcher = Regex::new(pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn los_title(html: &str) -> Option<String> {
    los_first_match(
        html,
        r#"(?is)<[^>]+\bid\s*=\s*["']programme-details["'][^>]*>.*?<h2\b[^>]*>(.*?)</h2>"#,
    )
    .or_else(|| {
        let matcher = Regex::new(
            r#"(?is)<[^>]+\bid\s*=\s*["']add-to-existing-playlist["'][^>]*\bdata-record-title\s*=\s*["']([^"']+)"#,
        )
        .ok()?;
        matcher
            .captures(html)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .filter(|value| !value.is_empty())
    })
}

fn los_duration(html: &str) -> Option<f64> {
    let raw = los_first_match(
        html,
        r#"(?is)\bclass\s*=\s*["'][^"']*\bprog-running-time\b[^"']*["'][^>]*>(.*?)</[^>]+>"#,
    )?;
    yt_dlp_core::parse_duration(raw.trim())
}

fn los_broadcast_date(html: &str) -> Option<String> {
    los_first_match(
        html,
        r#"(?is)\bclass\s*=\s*["'][^"']*\bbroadcast-date\b[^"']*["'][^>]*>([^<]+)"#,
    )
}

fn los_timestamp(value: &str) -> Option<i64> {
    if let Some(timestamp) = parse_timestamp(value.to_owned()) {
        return Some(timestamp);
    }
    let matcher = Regex::new(
        r#"(?i)(\d{1,2})(?:st|nd|rd|th)?\s+([a-z]{3,9})\s+(\d{4})(?:[^0-9]+(\d{1,2}):(\d{2})\s*(am|pm)?)?"#,
    )
    .ok()?;
    let captures = matcher.captures(value).ok().flatten()?;
    let month = match captures.get(2)?.as_str().to_ascii_lowercase().as_str() {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "may" => 5,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    };
    let day = captures.get(1)?.as_str().parse::<u32>().ok()?;
    let year = captures.get(3)?.as_str().parse::<u32>().ok()?;
    let mut hour = captures
        .get(4)
        .and_then(|value| value.as_str().parse::<u32>().ok())
        .unwrap_or(0);
    let minute = captures
        .get(5)
        .and_then(|value| value.as_str().parse::<u32>().ok())
        .unwrap_or(0);
    if captures
        .get(6)
        .is_some_and(|value| value.as_str().eq_ignore_ascii_case("pm"))
        && hour < 12
    {
        hour += 12;
    }
    if captures
        .get(6)
        .is_some_and(|value| value.as_str().eq_ignore_ascii_case("am"))
        && hour == 12
    {
        hour = 0;
    }
    if day == 0 || day > 31 || hour > 23 || minute > 59 {
        return None;
    }
    yt_dlp_core::parse_iso8601(&format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:00Z"
    ))
}

fn los_poster(page_url: &str, html: &str) -> Option<String> {
    let matcher =
        Regex::new(r#"(?is)<(?:video|audio)\b[^>]*\bposter\s*=\s*["']([^"']+)["']"#).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| resolve_url(page_url, &unescape_html_attribute(value.as_str())))
}
