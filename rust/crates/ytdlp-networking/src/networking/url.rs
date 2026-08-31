fn remove_dot_segments(path: &str) -> String {
    let leading_slash = path.starts_with('/');
    let trailing_slash = path.ends_with('/');
    let mut output = Vec::new();

    for segment in path.split('/') {
        match segment {
            "." | "" if leading_slash && output.is_empty() => {}
            "." | "" => output.push(segment),
            ".." => {
                if !output.is_empty() {
                    output.pop();
                }
            }
            segment => output.push(segment),
        }
    }

    let mut normalized = output.join("/");
    if leading_slash && !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }
    if trailing_slash && !normalized.ends_with('/') {
        normalized.push('/');
    }
    normalized
}

/// Normalize a URL using the RFC 3986 transformations used by yt-dlp.
pub fn normalize_url(input: &str) -> String {
    let input = input
        .strip_prefix("//")
        .map_or_else(|| input.to_owned(), |rest| format!("http://{rest}"));
    let Ok(mut parsed) = Url::parse(&input) else {
        return input;
    };

    let path = parsed.path().to_owned();
    let normalized_path = remove_dot_segments(&path);
    if normalized_path != path {
        parsed.set_path(&normalized_path);
    }

    remove_empty_authority_path(parsed.to_string(), &input, &path)
}

fn has_explicit_path(url: &str) -> bool {
    let Some(authority_start) = url.find("//") else {
        return true;
    };
    let after_authority = &url[authority_start + 2..];
    let authority_end = after_authority
        .find(|character| matches!(character, '/' | '?' | '#'))
        .unwrap_or(after_authority.len());
    after_authority[authority_end..].starts_with('/')
}

fn remove_empty_authority_path(mut url: String, original: &str, path: &str) -> String {
    // `url::Url` materializes `/` for an empty authority path, while Python's
    // urlunparse preserves `https://example.com` without that slash.
    if path != "/" || has_explicit_path(original) {
        return url;
    }

    let authority_start = url.find("://").map_or(0, |index| index + 3);
    if let Some(relative_end) =
        url[authority_start..].find(|character| matches!(character, '/' | '?' | '#'))
    {
        let path_index = authority_start + relative_end;
        if url.as_bytes().get(path_index) == Some(&b'/') {
            url.remove(path_index);
        }
    }
    url
}

/// Update existing query keys and append new keys in insertion order.
pub fn update_url_query(url: &str, query: &[(String, String)]) -> String {
    if query.is_empty() {
        return url.to_owned();
    }
    let normalized_url = normalize_url(url);
    let Ok(mut parsed) = Url::parse(&normalized_url) else {
        return normalized_url;
    };

    let mut pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();

    for (key, value) in query {
        if let Some(index) = pairs
            .iter()
            .position(|(existing_key, _)| existing_key == key)
        {
            pairs.retain(|(existing_key, _)| existing_key != key);
            pairs.insert(index, (key.clone(), value.clone()));
        } else {
            pairs.push((key.clone(), value.clone()));
        }
    }

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(&key, &value);
    }
    let encoded_query = serializer.finish();
    parsed.set_query((!encoded_query.is_empty()).then_some(encoded_query.as_str()));
    remove_empty_authority_path(parsed.to_string(), &normalized_url, parsed.path())
}
