fn murrtube_attribute(tag: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)\b{}\s*=\s*["']([^"']*)"#,
        regex::escape(name)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(tag).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| unescape_html_attribute(value.as_str())))
}

fn murrtube_hidden_form(homepage: &str) -> String {
    let Ok(input_matcher) = Regex::new(r#"(?is)<input\b[^>]*>"#) else {
        return String::new();
    };
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    for captures in input_matcher.captures_iter(homepage).flatten() {
        let Some(tag) = captures.get(0).map(|value| value.as_str()) else {
            continue;
        };
        if murrtube_attribute(tag, "type").is_some_and(|value| !value.eq_ignore_ascii_case("hidden")) {
            continue;
        }
        let (Some(name), Some(value)) = (
            murrtube_attribute(tag, "name"),
            murrtube_attribute(tag, "value"),
        ) else {
            continue;
        };
        form.append_pair(&name, &value);
    }
    form.finish()
}

fn murrtube_initialize(context: &ExtractionContext) -> Result<(), ExtractorError> {
    let homepage = context.get("https://murrtube.net")?;
    let homepage = String::from_utf8_lossy(homepage.body());
    let mut request = Request::new("https://murrtube.net/accept_age_check");
    request.set_data(Some(murrtube_hidden_form(&homepage).into_bytes()));
    context.request(&request)?;
    Ok(())
}

fn murrtube_page(
    context: &ExtractionContext,
    url: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(url)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}
