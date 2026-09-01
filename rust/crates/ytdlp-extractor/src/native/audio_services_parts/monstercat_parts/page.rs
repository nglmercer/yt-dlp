fn monstercat_class_values(html: &str, class_name: &str) -> Vec<String> {
    let pattern = format!(
        r#"(?is)<(?P<tag>[a-z0-9]+)\b[^>]*\bclass\s*=\s*[\"'][^\"']*\b{}\b[^\"']*[\"'][^>]*>(.*?)</(?P=tag)\s*>"#,
        regex::escape(class_name)
    );
    let Ok(matcher) = Regex::new(&pattern) else {
        return Vec::new();
    };
    matcher
        .captures_iter(html)
        .flatten()
        .filter_map(|captures| captures.get(2).map(|value| value.as_str().to_owned()))
        .collect()
}

fn monstercat_release_date(value: &str) -> Option<String> {
    let value = html_text_fragment(value);
    let month_number = |month: &str| -> Option<u32> {
        match month.to_ascii_lowercase().as_str() {
            "jan" | "january" => Some(1),
            "feb" | "february" => Some(2),
            "mar" | "march" => Some(3),
            "apr" | "april" => Some(4),
            "may" => Some(5),
            "jun" | "june" => Some(6),
            "jul" | "july" => Some(7),
            "aug" | "august" => Some(8),
            "sep" | "sept" | "september" => Some(9),
            "oct" | "october" => Some(10),
            "nov" | "november" => Some(11),
            "dec" | "december" => Some(12),
            _ => None,
        }
    };
    let month_name = r#"Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:t(?:ember)?)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?"#;
    for pattern in [
        format!(r#"(?ix)\b(?P<month>{month_name})\s+(?P<day>\d{{1,2}}),?\s+(?P<year>\d{{4}})\b"#),
        format!(r#"(?ix)\b(?P<day>\d{{1,2}})\s+(?P<month>{month_name})\s+(?P<year>\d{{4}})\b"#),
    ] {
        let Ok(matcher) = Regex::new(&pattern) else {
            continue;
        };
        let Some(captures) = matcher.captures(&value).ok().flatten() else {
            continue;
        };
        let month = captures
            .name("month")
            .and_then(|value| month_number(value.as_str()))?;
        let day = captures
            .name("day")
            .and_then(|value| value.as_str().parse::<u32>().ok())?;
        let year = captures
            .name("year")
            .and_then(|value| value.as_str().parse::<u32>().ok())?;
        if (1..=31).contains(&day) && (1..=12).contains(&month) {
            return Some(format!("{year:04}{month:02}{day:02}"));
        }
    }

    let iso_matcher = Regex::new(
        r#"\b(?P<year>\d{4})[-/]?(?P<month>\d{2})[-/]?(?P<day>\d{2})\b"#,
    )
    .ok()?;
    let captures = iso_matcher.captures(&value).ok().flatten()?;
    let year = captures.name("year")?.as_str();
    let month = captures.name("month")?.as_str();
    let day = captures.name("day")?.as_str();
    Some(format!("{year}{month}{day}"))
}

fn monstercat_album_meta(html: &str, release_id: &str) -> InfoDict {
    let title = monstercat_first_element_text(html, "h1");
    let album_artists = monstercat_class_values(
        html,
        "h-normal text-uppercase mb-desktop-medium mb-smallish",
    )
    .into_iter()
    .map(|value| html_text_fragment(&value))
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();
    let release_date = monstercat_class_values(
        html,
        "font-italic mb-medium d-tablet-none d-phone-block",
    )
    .into_iter()
    .find_map(|value| {
        let value = html_text_fragment(&value);
        let date = value
            .split_once("Released ")
            .map_or(value.as_str(), |(_, date)| date);
        monstercat_release_date(date)
    });

    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(release_id));
    info.insert_if_some("title", title.clone());
    info.insert("album", serde_json::json!(title));
    info.insert(
        "thumbnail",
        serde_json::json!(format!("https://www.monstercat.com/release/{release_id}/cover")),
    );
    info.insert_if_some("album_artists", (!album_artists.is_empty()).then_some(album_artists));
    info.insert_if_some("release_date", release_date);
    info
}

fn monstercat_first_element_text(html: &str, tag: &str) -> Option<String> {
    let pattern = format!(
        r"(?is)<{tag}\b[^>]*>(.*?)</{tag}\s*>",
        tag = regex::escape(tag)
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
