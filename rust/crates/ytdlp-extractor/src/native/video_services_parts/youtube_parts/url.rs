fn youtube_valid_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// Match a playlist ID at the start of `value`, mirroring `re.match` with
/// `_PLAYLIST_ID_RE` (`(?:PL|LL|EC|UU|FL|RD|UL|TL|PU|OLAK5uy_)[0-9A-Za-z-_]{10,}`
/// or a bare short ID). The match is a prefix: trailing garbage is ignored,
/// exactly like the Python ID group.
fn youtube_playlist_id_prefix(value: &str) -> Option<String> {
    const LONG_PREFIXES: &[&str] = &[
        "PL", "LL", "EC", "UU", "FL", "RD", "UL", "TL", "PU", "OLAK5uy_",
    ];
    for prefix in LONG_PREFIXES {
        if let Some(rest) = value.strip_prefix(prefix) {
            let id_chars = rest
                .bytes()
                .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                .count();
            if id_chars >= 10 {
                return Some(format!("{prefix}{}", &rest[..id_chars]));
            }
        }
    }
    for short in ["RDMM", "WL", "LL", "LM"] {
        if value.starts_with(short) {
            return Some(short.to_owned());
        }
    }
    None
}

/// Extract a playlist ID from playlist URLs and bare IDs, mirroring the
/// intake half of `YoutubePlaylistIE` (`/playlist?list=`, `/watch?list=`
/// without `v`, `/embed/videoseries?list=`, and bare IDs). Watch URLs with a
/// video ID, channel/tab/search/feed URLs, and non-YouTube hosts never match:
/// they keep their existing video, tab-TODO, or generic routing.
pub(crate) fn youtube_playlist_id(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let parsed = if trimmed.starts_with("//") {
        url::Url::parse(&format!("https:{trimmed}")).ok()
    } else {
        url::Url::parse(trimmed).ok()
    };
    let Some(parsed) = parsed else {
        return youtube_playlist_id_prefix(trimmed);
    };
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
    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let param = |key: &str| {
        pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    if first.eq_ignore_ascii_case("playlist") {
        return param("list").and_then(youtube_playlist_id_prefix);
    }
    if first.eq_ignore_ascii_case("watch") {
        // Mirrors the "video URL given without video ID" branch: a bare
        // `watch?list=` falls back to the playlist flow, while `watch?v=`
        // stays on the video flow.
        if param("v").is_some() {
            return None;
        }
        return param("list").and_then(youtube_playlist_id_prefix);
    }
    if first.eq_ignore_ascii_case("embed")
        && segments
            .get(1)
            .is_some_and(|second| second.eq_ignore_ascii_case("videoseries"))
    {
        return param("list").and_then(youtube_playlist_id_prefix);
    }
    None
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

/// The claimed parts of a `YoutubeTabIE` URL, mirroring `_get_url_mobj`:
/// `pre`/`tab`/`post` split the URL around the optional `/tab` segment,
/// `not_channel` marks feed/hashtag/`playlist?list=`/`watch?list=` URLs,
/// `channel_type` is the channel path kind, and `id` the channel/feed/list
/// identifier. Missing groups are empty strings, never absent.
pub(crate) struct YoutubeTabUrl {
    pub pre: String,
    pub tab: String,
    pub post: String,
    pub not_channel: String,
    pub channel_type: String,
    pub id: String,
}

fn youtube_tab_url_pattern() -> Option<fancy_regex::Regex> {
    // Mirrors `_URL_RE` (`_VALID_URL` plus the conditional `/tab` group)
    // for official YouTube hosts. Invidious hosts are intentionally not
    // claimed: the port's routing policy (`youtube_official_host`) only
    // serves official hosts, matching the video/playlist intake.
    const RESERVED: &str = "channel|c|user|playlist|watch|w|v|embed|e|live|watch_popup|clip|\
        shorts|movies|results|search|shared|hashtag|trending|explore|feed|feeds|\
        browse|oembed|get_video_info|iframe_api|s/player|source|\
        storefront|oops|index|account|t/terms|about|upload|signin|logout";
    fancy_regex::Regex::new(&format!(
        "\\A(?P<pre>https?://(?!consent\\.)(?:\\w+\\.)?youtube(?:kids)?\\.com/\
        (?:(?P<channel_type>channel|c|user|browse)/\
        |(?P<not_channel>feed/|hashtag/|(?:playlist|watch)\\?.*?\\blist=)\
        |(?!(?:{RESERVED})\\b))\
        (?P<id>[^/?#&]+))\
        (?(not_channel)|(?P<tab>/[^?#/]+))?(?P<post>.*)$"
    ))
    .ok()
}

fn youtube_tab_group(captures: &fancy_regex::Captures, name: &str) -> String {
    captures
        .name(name)
        .map(|part| part.as_str().to_owned())
        .unwrap_or_default()
}

/// Split a tab URL the way `_get_url_mobj` does, or `None` when the URL is
/// not tab-shaped.
pub(crate) fn youtube_tab_url_parts(url: &str) -> Option<YoutubeTabUrl> {
    let captures = youtube_tab_url_pattern()?.captures(url).ok().flatten()?;
    Some(YoutubeTabUrl {
        pre: youtube_tab_group(&captures, "pre"),
        tab: youtube_tab_group(&captures, "tab"),
        post: youtube_tab_group(&captures, "post"),
        not_channel: youtube_tab_group(&captures, "not_channel"),
        channel_type: youtube_tab_group(&captures, "channel_type"),
        id: youtube_tab_group(&captures, "id"),
    })
}

/// Mirror `YoutubeTabIE.suitable`: claimable when no video extractor takes
/// the URL and the tab pattern matches.
pub(crate) fn youtube_tab_suitable(url: &str) -> bool {
    youtube_video_id(url).is_none() && youtube_tab_url_parts(url).is_some()
}

fn youtube_query_value(url: &str, key: &str) -> Option<String> {
    url::Url::parse(url).ok()?.query_pairs().find_map(|(name, value)| {
        (name == key).then(|| value.into_owned())
    })
}

/// Remove every instance of a query parameter, mirroring
/// `update_url_query` with an empty value list (used for `xosf`).
fn youtube_strip_query_param(url: &str, key: &str) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    let retained = parsed
        .query_pairs()
        .filter(|(name, _)| name != key)
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    parsed.query_pairs_mut().clear();
    {
        let mut query = parsed.query_pairs_mut();
        for (name, value) in &retained {
            query.append_pair(name, value);
        }
    }
    Some(parsed.into())
}

/// Replace (or append) a single query parameter, mirroring
/// `update_url_query` replacement semantics for solved challenges.
fn youtube_replace_query(url: &str, key: &str, value: &str) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    let retained = parsed
        .query_pairs()
        .filter(|(name, _)| name != key)
        .map(|(name, val)| (name.into_owned(), val.into_owned()))
        .collect::<Vec<_>>();
    parsed.query_pairs_mut().clear();
    {
        let mut query = parsed.query_pairs_mut();
        for (name, val) in &retained {
            query.append_pair(name, val);
        }
        query.append_pair(key, value);
    }
    Some(parsed.into())
}

/// Replace an `/n/<challenge>/` path segment, mirroring the manifest
/// `n`-challenge rewrite in `process_manifest_format`.
fn youtube_replace_n_path_segment(url: &str, challenge: &str, result: &str) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    let path = parsed.path().to_owned();
    let needle = format!("/n/{challenge}");
    let replacement = format!("/n/{result}");
    let updated = path.replace(&needle, &replacement);
    (updated != path).then(|| {
        parsed.set_path(&updated);
        parsed.into()
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
