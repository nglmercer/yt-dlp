fn youtube_valid_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn youtube_official_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    matches!(
        host.as_str(),
        "youtu.be" | "youtube.com" | "youtube-nocookie.com" | "youtube-kids.com"
    ) || host.ends_with(".youtube.com")
        || host.ends_with(".youtube-nocookie.com")
        || host.ends_with(".youtube-kids.com")
}

pub(crate) fn youtube_video_id(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if youtube_valid_video_id(trimmed) {
        return Some(trimmed.to_owned());
    }

    let parse_target = if trimmed.starts_with("//") {
        format!("https:{trimmed}")
    } else {
        trimmed.to_owned()
    };
    let parsed = url::Url::parse(&parse_target).ok()?;
    let host = parsed.host_str()?;
    if !youtube_official_host(host) {
        return None;
    }

    let segments = parsed
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let first = segments.first().copied().unwrap_or_default();
    if host.eq_ignore_ascii_case("youtu.be") {
        return segments
            .first()
            .filter(|id| youtube_valid_video_id(id))
            .map(|id| (*id).to_owned());
    }

    if first.eq_ignore_ascii_case("watch") || first.eq_ignore_ascii_case("movie") {
        return parsed
            .query_pairs()
            .find(|(key, _)| key == "v")
            .map(|(_, value)| value.into_owned())
            .filter(|id| youtube_valid_video_id(id));
    }

    if matches!(
        first.to_ascii_lowercase().as_str(),
        "shorts" | "embed" | "v" | "live"
    ) {
        return segments
            .get(1)
            .filter(|id| youtube_valid_video_id(id))
            .map(|id| (*id).to_owned());
    }

    None
}

fn youtube_canonical_url(video_id: &str) -> String {
    format!("https://www.youtube.com/watch?v={video_id}")
}

fn youtube_query_value(url: &str, key: &str) -> Option<String> {
    url::Url::parse(url).ok()?.query_pairs().find_map(|(name, value)| {
        (name == key).then(|| value.into_owned())
    })
}

fn youtube_update_query(url: &str, values: &[(&str, &str)]) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    {
        let mut query = parsed.query_pairs_mut();
        for (key, value) in values {
            query.append_pair(key, value);
        }
    }
    Some(parsed.into())
}

fn youtube_url_has_n_challenge(url: &str) -> bool {
    if youtube_query_value(url, "n").is_some() {
        return true;
    }
    let Some(path) = url::Url::parse(url).ok().map(|url| url.path().to_owned()) else {
        return false;
    };
    let segments = path.split('/').collect::<Vec<_>>();
    segments
        .windows(2)
        .any(|window| window[0] == "n" && !window[1].is_empty())
}
