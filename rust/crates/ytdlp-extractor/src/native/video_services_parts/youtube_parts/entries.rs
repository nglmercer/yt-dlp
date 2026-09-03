/// Native port of the `YoutubeTabIE` video-entry helpers in
/// `yt_dlp/extractor/youtube/_tab.py` (with `_get_text`, badges, thumbnails,
/// and availability from `_base.py` and `parse_count`/`parse_duration` from
/// `yt_dlp/utils/_utils.py`).
///
/// This covers the deterministic offline slice: renderer text extraction,
/// badge classification, thumbnail shaping, view-count and duration parsing,
/// channel UCID/handle validation, and the full `_extract_video`
/// `videoRenderer` mapping. Tab URL claiming, section dispatch, browse
/// continuations, and the `_real_extract` composition stay TODOs.
///
/// Paths use `"..."` for the `traverse_obj` Ellipsis step; every other
/// segment is a literal object key.
use std::sync::OnceLock;

/// Walk one key/`...` path, mirroring the list-producing `traverse_obj`
/// steps used by the entry helpers. Like `traverse_obj`, `...` expands
/// both arrays and mapping values; mapping order follows
/// `serde_json::Map` (sorted: the workspace does not enable
/// `preserve_order`), which matches source order for the single-key
/// containers renderers emit in practice.
fn youtube_traverse_nodes<'v>(
    value: &'v serde_json::Value,
    path: &[&str],
) -> Vec<&'v serde_json::Value> {
    let mut current = vec![value];
    for segment in path {
        let mut next = Vec::new();
        for node in current {
            if *segment == "..." {
                match node {
                    serde_json::Value::Array(items) => next.extend(items.iter()),
                    serde_json::Value::Object(map) => next.extend(map.values()),
                    _ => {}
                }
            } else if let Some(child) = node.get(*segment) {
                next.push(child);
            }
        }
        current = next;
        if current.is_empty() {
            break;
        }
    }
    current
}

/// First node over several candidate paths, mirroring the
/// `get_all=False` traversals.
pub(crate) fn youtube_traverse_first<'v>(
    data: &'v serde_json::Value,
    paths: &[&[&str]],
) -> Option<&'v serde_json::Value> {
    paths
        .iter()
        .flat_map(|path| youtube_traverse_nodes(data, path))
        .next()
}

/// Read one text node the way `_get_text` does: a non-empty `simpleText`
/// wins, otherwise the `runs` texts are joined. Bare strings never count,
/// matching `try_get(item, lambda x: x['simpleText'], str)` on non-objects.
fn youtube_textish(node: &serde_json::Value) -> Option<String> {
    if let Some(text) = node
        .get("simpleText")
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
    {
        return Some(text.to_owned());
    }
    let runs = node.get("runs")?.as_array()?;
    let text = runs
        .iter()
        .filter_map(|run| run.get("text").and_then(serde_json::Value::as_str))
        .collect::<String>();
    (!text.is_empty()).then_some(text)
}

/// First non-empty renderer text over several candidate paths, mirroring
/// `_get_text(data, *path_list)`.
pub(crate) fn youtube_renderer_text(data: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        for node in youtube_traverse_nodes(data, path) {
            if let Some(items) = node.as_array() {
                // A bare runs list, mirroring `runs = item` in `_get_text`.
                for item in items {
                    if let Some(text) = youtube_textish(item) {
                        return Some(text);
                    }
                }
            } else if let Some(text) = youtube_textish(node) {
                return Some(text);
            }
        }
    }
    None
}

/// First string node over several candidate paths, mirroring the
/// `expected_type=str, get_all=False` traversals.
pub(crate) fn youtube_first_str(data: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .flat_map(|path| youtube_traverse_nodes(data, path))
        .find_map(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// Badge classes from `_base.BadgeType`, limited to the variants the tab
/// entries read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum YoutubeBadge {
    AvailabilityUnlisted,
    AvailabilityPrivate,
    AvailabilityPublic,
    AvailabilityPremium,
    AvailabilitySubscription,
    LiveNow,
    Verified,
}

/// The `*badgeRenderer` children of a badge list, mirroring the
/// `(..., lambda key, _: re.search(r'[bB]adgeRenderer$', key))` step of
/// `_extract_badges`.
fn youtube_badge_renderers<'v>(badge_list: &'v serde_json::Value) -> Vec<&'v serde_json::Value> {
    let items: Vec<&'v serde_json::Value> = match badge_list.as_array() {
        Some(items) => items.iter().collect(),
        None => vec![badge_list],
    };
    let mut renderers = Vec::new();
    for item in items {
        let serde_json::Value::Object(map) = item else {
            continue;
        };
        for (key, renderer) in map {
            if (key.ends_with("badgeRenderer") || key.ends_with("BadgeRenderer"))
                && renderer.is_object()
            {
                renderers.push(renderer);
            }
        }
    }
    renderers
}

fn youtube_icon_badge(icon_type: &str) -> Option<YoutubeBadge> {
    match icon_type {
        "PRIVACY_UNLISTED" => Some(YoutubeBadge::AvailabilityUnlisted),
        "PRIVACY_PRIVATE" => Some(YoutubeBadge::AvailabilityPrivate),
        "PRIVACY_PUBLIC" => Some(YoutubeBadge::AvailabilityPublic),
        "CHECK_CIRCLE_THICK" | "OFFICIAL_ARTIST_BADGE" | "CHECK" => Some(YoutubeBadge::Verified),
        _ => None,
    }
}

fn youtube_style_badge(style: &str) -> Option<YoutubeBadge> {
    match style {
        "BADGE_STYLE_TYPE_MEMBERS_ONLY" => Some(YoutubeBadge::AvailabilitySubscription),
        "BADGE_STYLE_TYPE_PREMIUM" => Some(YoutubeBadge::AvailabilityPremium),
        "BADGE_STYLE_TYPE_LIVE_NOW" => Some(YoutubeBadge::LiveNow),
        "BADGE_STYLE_TYPE_VERIFIED" | "BADGE_STYLE_TYPE_VERIFIED_ARTIST" => {
            Some(YoutubeBadge::Verified)
        }
        _ => None,
    }
}

/// Classify a badge list, mirroring `_extract_badges` including the
/// insertion-ordered label fallback.
pub(crate) fn youtube_badges(badge_list: &serde_json::Value) -> Vec<YoutubeBadge> {
    let mut badges = Vec::new();
    for badge in youtube_badge_renderers(badge_list) {
        let mapped = badge
            .get("icon")
            .and_then(|icon| icon.get("iconType"))
            .and_then(serde_json::Value::as_str)
            .and_then(youtube_icon_badge)
            .or_else(|| {
                badge
                    .get("style")
                    .and_then(serde_json::Value::as_str)
                    .and_then(youtube_style_badge)
            });
        if let Some(badge_type) = mapped {
            badges.push(badge_type);
            continue;
        }
        // Fallback label scan; documented as not working in some languages.
        let label = badge
            .get("label")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                badge
                    .get("accessibilityData")
                    .and_then(|data| data.get("label"))
                    .and_then(serde_json::Value::as_str)
            })
            .or_else(|| badge.get("tooltip").and_then(serde_json::Value::as_str))
            .or_else(|| badge.get("iconTooltip").and_then(serde_json::Value::as_str))
            .unwrap_or("")
            .to_lowercase();
        for (needle, badge_type) in [
            ("unlisted", YoutubeBadge::AvailabilityUnlisted),
            ("private", YoutubeBadge::AvailabilityPrivate),
            ("members only", YoutubeBadge::AvailabilitySubscription),
            ("live", YoutubeBadge::LiveNow),
            ("premium", YoutubeBadge::AvailabilityPremium),
            ("verified", YoutubeBadge::Verified),
            ("official artist channel", YoutubeBadge::Verified),
        ] {
            if label.contains(needle) {
                badges.push(badge_type);
                break;
            }
        }
    }
    badges
}

/// Mirror `_has_badge`.
pub(crate) fn youtube_has_badge(badges: &[YoutubeBadge], badge: YoutubeBadge) -> bool {
    badges.contains(&badge)
}

/// The URL schemes `url_or_none` accepts, spelling out
/// `(?:https?|rtm(?:pt?[es]?|fp)|ftps?|wss?)`.
const YOUTUBE_URL_SCHEMES: [&str; 12] = [
    "http", "https", "rtmp", "rtmpe", "rtmps", "rtmpt", "rtmpte", "rtmpts", "rtmfp", "ftp", "ftps",
    "wss",
];

/// Mirror `url_or_none` over a string.
fn youtube_url_str_or_none(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    if url.starts_with("//") {
        return Some(url.to_owned());
    }
    let (scheme, _) = url.split_once("://")?;
    YOUTUBE_URL_SCHEMES
        .contains(&scheme)
        .then(|| url.to_owned())
}

/// Mirror `url_or_none` for thumbnail URLs.
fn youtube_url_or_none(url: &serde_json::Value) -> Option<String> {
    youtube_url_str_or_none(url.as_str()?)
}

/// Mirror `int_or_none` for the numeric-or-string thumbnail dimensions and
/// `lengthSeconds`.
pub(crate) fn youtube_int_or_none(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// Shape one thumbnail dict, mirroring `_extract_thumbnails` including the
/// `maxresdefault` query strip.
fn youtube_shape_thumbnail(thumbnail: &serde_json::Value) -> Option<serde_json::Value> {
    let mut url = youtube_url_or_none(thumbnail.get("url")?)?;
    // YouTube sometimes serves a wrong thumbnail URL; see yt-dlp#233.
    if url.contains("maxresdefault") {
        url = url.split('?').next().unwrap_or(&url).to_owned();
    }
    Some(serde_json::json!({
        "url": url,
        "height": thumbnail.get("height").and_then(youtube_int_or_none)
            .map(serde_json::Value::from).unwrap_or(serde_json::Value::Null),
        "width": thumbnail.get("width").and_then(youtube_int_or_none)
            .map(serde_json::Value::from).unwrap_or(serde_json::Value::Null),
    }))
}

/// Extract thumbnails at one key path with a custom final key, mirroring
/// `_extract_thumbnails(data, *path_list, final_key=...)`.
pub(crate) fn youtube_thumbnails_at(
    data: &serde_json::Value,
    path: &[&str],
    final_key: &str,
) -> Vec<serde_json::Value> {
    let mut full: Vec<&str> = path.to_vec();
    full.extend([final_key, "..."]);
    youtube_traverse_nodes(data, &full)
        .into_iter()
        .filter_map(youtube_shape_thumbnail)
        .collect()
}

/// Extract the thumbnails under one key, mirroring
/// `_extract_thumbnails(data, key)`.
pub(crate) fn youtube_entry_thumbnails(data: &serde_json::Value) -> Vec<serde_json::Value> {
    youtube_thumbnails_at(data, &["thumbnail"], "thumbnails")
}

/// Mirror `str_to_int`: plain integers pass through, strings lose every
/// `,.+` before parsing.
pub(crate) fn youtube_str_to_int_text(text: &str) -> Option<i64> {
    text.chars()
        .filter(|c| !matches!(c, ',' | '.' | '+'))
        .collect::<String>()
        .parse()
        .ok()
}

/// Mirror `str_to_int` over a JSON number-or-string.
pub(crate) fn youtube_str_to_int(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(text) => youtube_str_to_int_text(text),
        _ => None,
    }
}

fn youtube_count_junk_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    // Mirrors `re.sub(r'^[^\d]+\s', '', s)`.
    PATTERN.get_or_init(|| regex::Regex::new(r"^[^\d]+\s").expect("static count pattern"))
}

fn youtube_count_head_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    // Mirrors `re.match(r'([\d,.]+)(?:$|\s)', s)`: leading digits run whose
    // next character ends the string or is whitespace.
    PATTERN.get_or_init(|| regex::Regex::new(r"^[\d,.]+").expect("static count pattern"))
}

/// Mirror the non-strict `lookup_unit_table` over the count unit table,
/// keeping the alternation order (`k` before `kk`) and the `\b` check.
fn youtube_lookup_count_unit(text: &str) -> Option<i64> {
    static NUMBERS: OnceLock<regex::Regex> = OnceLock::new();
    let numbers =
        NUMBERS.get_or_init(|| regex::Regex::new(r"^\d+(?:[,.]\d+)?").expect("static num pattern"));
    let digits = numbers.find(text)?.as_str();
    let rest = text[digits.len()..].trim_start();
    for (unit, mult) in [
        ("k", 1_000.0),
        ("K", 1_000.0),
        ("m", 1_000_000.0),
        ("M", 1_000_000.0),
        ("kk", 1_000_000.0),
        ("KK", 1_000_000.0),
        ("b", 1_000_000_000.0),
        ("B", 1_000_000_000.0),
    ] {
        if let Some(after) = rest.strip_prefix(unit) {
            let boundary = after
                .chars()
                .next()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            if boundary {
                let num: f64 = digits.replace(',', ".").parse().ok()?;
                return Some((num * mult).round() as i64);
            }
        }
    }
    None
}

/// Mirror `parse_count`.
pub(crate) fn youtube_parse_count(text: &str) -> Option<i64> {
    let stripped = youtube_count_junk_pattern()
        .replace(text, "")
        .trim()
        .to_owned();
    if !stripped.is_empty()
        && stripped
            .chars()
            .all(|c| c.is_ascii_digit() || c == ',' || c == '.')
    {
        return youtube_str_to_int_text(&stripped);
    }
    if let Some(count) = youtube_lookup_count_unit(&stripped) {
        return Some(count);
    }
    let head = youtube_count_head_pattern().find(&stripped)?;
    let after = &stripped[head.end()..];
    if after.is_empty() || after.starts_with(char::is_whitespace) {
        youtube_str_to_int_text(head.as_str())
    } else {
        None
    }
}

/// Mirror `_get_count` over one already-read count text.
fn youtube_count_text_value(text: &str) -> Option<i64> {
    if text.to_lowercase().starts_with("no ") {
        return Some(0);
    }
    if let Some(count) = youtube_parse_count(text) {
        return Some(count);
    }
    // Fallback: leading `[\d,]+` prefix run of the whitespace-squeezed
    // text, with no trailing constraint.
    let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let head: String = squeezed
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .collect();
    if head.is_empty() {
        return None;
    }
    youtube_str_to_int_text(&head)
}

/// Mirror `_get_count` over renderer text paths.
pub(crate) fn youtube_get_count(data: &serde_json::Value, paths: &[&[&str]]) -> Option<i64> {
    youtube_count_text_value(&youtube_renderer_text(data, paths).unwrap_or_default())
}

/// Compile one `parse_duration` alternative anchored at the string start
/// (`re.match` semantics); the alternatives already carry their own `$`.
fn youtube_duration_pattern(source: &str) -> Option<fancy_regex::Regex> {
    fancy_regex::Regex::new(&format!("\\A(?:{source})")).ok()
}

fn youtube_duration_part(captures: &fancy_regex::Captures, name: &str) -> f64 {
    captures
        .name(name)
        .map(|part| part.as_str().replace(':', "."))
        .and_then(|part| part.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn youtube_duration_total(captures: &fancy_regex::Captures) -> f64 {
    youtube_duration_part(captures, "days") * 86_400.0
        + youtube_duration_part(captures, "hours") * 3_600.0
        + youtube_duration_part(captures, "mins") * 60.0
        + youtube_duration_part(captures, "secs")
        + youtube_duration_part(captures, "ms")
}

/// Mirror `parse_duration`: colon stamps, ISO-ish spans, then bare
/// hour/minute words. Only the `ms` group swaps `:` for `.` before the
/// float conversion.
pub(crate) fn youtube_parse_duration(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // The first alternative uses a `(?(name)...)` conditional, so it needs
    // the fancy engine; the others match either way.
    let timestamp = r"(?x)
            (?P<before_secs>
                (?:(?:(?P<days>[0-9]+):)?(?P<hours>[0-9]+):)?(?P<mins>[0-9]+):)?
            (?P<secs>(?(before_secs)[0-9]{1,2}|[0-9]+))
            (?P<ms>[.:][0-9]+)?Z?$";
    if let Some(captures) = youtube_duration_pattern(timestamp)
        .and_then(|pattern| pattern.captures(text).ok().flatten())
    {
        return Some(youtube_duration_total(&captures));
    }
    let iso = r"(?ix)(?:P?
                (?:
                    [0-9]+\s*y(?:ears?)?,?\s*
                )?
                (?:
                    [0-9]+\s*m(?:onths?)?,?\s*
                )?
                (?:
                    [0-9]+\s*w(?:eeks?)?,?\s*
                )?
                (?:
                    (?P<days>[0-9]+)\s*d(?:ays?)?,?\s*
                )?
                T)?
                (?:
                    (?P<hours>[0-9]+)\s*h(?:(?:ou)?rs?)?,?\s*
                )?
                (?:
                    (?P<mins>[0-9]+)\s*m(?:in(?:ute)?s?)?,?\s*
                )?
                (?:
                    (?P<secs>[0-9]+)(?P<ms>\.[0-9]+)?\s*s(?:ec(?:ond)?s?)?\s*
                )?Z?$";
    if let Some(captures) =
        youtube_duration_pattern(iso).and_then(|pattern| pattern.captures(text).ok().flatten())
    {
        return Some(youtube_duration_total(&captures));
    }
    let words =
        r"(?i)(?:(?P<hours>[0-9.]+)\s*(?:hours?)|(?P<mins>[0-9.]+)\s*(?:mins?\.?|minutes?)\s*)Z?$";
    let captures = youtube_duration_pattern(words)
        .and_then(|pattern| pattern.captures(text).ok().flatten())?;
    Some(
        youtube_duration_part(&captures, "hours") * 3_600.0
            + youtube_duration_part(&captures, "mins") * 60.0,
    )
}

/// Serialize a duration the way the oracle prints it: integral totals stay
/// integers, fractional ones stay floats.
fn youtube_duration_number(total: f64) -> serde_json::Value {
    if total.is_finite()
        && total.fract() == 0.0
        && total >= i64::MIN as f64
        && total <= i64::MAX as f64
    {
        serde_json::Value::from(total as i64)
    } else {
        serde_json::Number::from_f64(total)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)
    }
}

fn youtube_ucid_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| regex::Regex::new(r"^UC[\w-]{22}$").expect("static ucid pattern"))
}

/// Mirror `ucid_or_none`: `None` in means `None` out, never a search error.
pub(crate) fn youtube_ucid(ucid: Option<&str>) -> Option<String> {
    let ucid = ucid?;
    youtube_ucid_pattern()
        .is_match(ucid)
        .then(|| ucid.to_owned())
}

fn youtube_handle_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"^(?:https?://(?:www\.)?youtube\.com)?/(@[\w.-]{3,30})")
            .expect("static handle pattern")
    })
}

/// Percent-decode like `urllib.parse.unquote` (plus signs stay literal).
fn youtube_percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len() + 1
            && bytes[index + 1..]
                .iter()
                .take(2)
                .all(|byte| byte.is_ascii_hexdigit())
        {
            let hex = &text[index + 1..index + 3];
            decoded.push(u8::from_str_radix(hex, 16).unwrap_or(b'%'));
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Mirror `handle_from_url`.
pub(crate) fn youtube_handle_from_url(url: Option<&str>) -> Option<String> {
    let decoded = youtube_percent_decode(url.unwrap_or(""));
    youtube_handle_pattern()
        .captures(&decoded)
        .and_then(|captures| captures.get(1))
        .map(|handle| handle.as_str().to_owned())
}

/// Mirror `InfoExtractor._availability`. `needs_auth` stays `None` at the
/// tab call site, so `all_known` never fires there; it is kept explicit so
/// the chain matches the source branch for branch.
pub(crate) fn youtube_availability(
    is_private: Option<bool>,
    needs_premium: Option<bool>,
    needs_subscription: Option<bool>,
    needs_auth: Option<bool>,
    is_unlisted: Option<bool>,
) -> Option<&'static str> {
    if is_private == Some(true) {
        Some("private")
    } else if needs_premium == Some(true) {
        Some("premium_only")
    } else if needs_subscription == Some(true) {
        Some("subscriber_only")
    } else if needs_auth == Some(true) {
        Some("needs_auth")
    } else if is_unlisted == Some(true) {
        Some("unlisted")
    } else if is_private.is_some()
        && needs_premium.is_some()
        && needs_subscription.is_some()
        && needs_auth.is_some()
        && is_unlisted.is_some()
    {
        Some("public")
    } else {
        None
    }
}

fn youtube_duration_label_pattern() -> Option<fancy_regex::Regex> {
    // Last-`ago` duration fallback for Shorts labels, verbatim from
    // `_extract_video` (needs the backreference, so fancy again).
    fancy_regex::Regex::new(
        r"(?i)(ago)(?!.*\1)\s+(?P<duration>[a-z0-9 ,]+?)(?:\s+[\d,]+\s+views)?(?:\s+-\s+play\s+short)?$",
    )
    .ok()
}

/// Read the Shorts `reelPlayerHeaderRenderer`, if the renderer navigates to
/// one.
/// Build a `url_result`-shaped entry: `{_type: url, url, ie_key}` plus the
/// video `id` and `title` when known.
pub(crate) fn youtube_url_entry(
    url: &str,
    ie_key: &str,
    id: Option<&str>,
    title: Option<String>,
) -> InfoDict {
    let mut entry = InfoDict::new();
    entry.insert("_type", serde_json::json!("url"));
    entry.insert("url", serde_json::json!(url));
    entry.insert("ie_key", serde_json::json!(ie_key));
    entry.insert_if_some("id", id);
    entry.insert_if_some("title", title);
    entry
}

/// Join a YouTube endpoint reference onto a base URL, mirroring
/// `urljoin` for the absolute and root-relative references renderers emit.
pub(crate) fn youtube_join_url(base: &str, reference: &str) -> String {
    if reference.contains("://") || reference.starts_with("//") {
        reference.to_owned()
    } else if let Some(path) = reference.strip_prefix('/') {
        let origin = base
            .split_once("://")
            .map(|(scheme, rest)| {
                let host = rest.split('/').next().unwrap_or(rest);
                format!("{scheme}://{host}")
            })
            .unwrap_or_else(|| base.to_owned());
        format!("{origin}/{path}")
    } else if base.ends_with('/') {
        format!("{base}{reference}")
    } else {
        format!("{base}/{reference}")
    }
}

fn youtube_reel_header(renderer: &serde_json::Value) -> Option<&serde_json::Value> {
    youtube_traverse_nodes(
        renderer,
        &[
            "navigationEndpoint",
            "reelWatchEndpoint",
            "overlay",
            "reelPlayerOverlayRenderer",
            "reelPlayerHeaderSupportedRenderers",
            "reelPlayerHeaderRenderer",
        ],
    )
    .into_iter()
    .next()
}

/// Build one tab entry from a `videoRenderer`, mirroring `_extract_video`.
///
/// Renderers without a video ID are skipped, matching the playlist-entry
/// convention (`_playlist_entries` drops ID-less children; the Python
/// method would emit an `id: None` dict the playlist flow cannot use).
/// `timestamp` stays unset: the source only fills it behind the opt-in
/// `approximate_date` extractor argument, whose relative-time parser is a
/// TODO.
pub(crate) fn youtube_extract_video(renderer: &serde_json::Value) -> Option<InfoDict> {
    let video_id = renderer
        .get("videoId")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())?;
    let reel = youtube_reel_header(renderer);

    let title = youtube_renderer_text(renderer, &[&["title"], &["headline"]])
        .or_else(|| reel.and_then(|header| youtube_renderer_text(header, &[&["reelTitleText"]])));
    let description = youtube_renderer_text(
        renderer,
        &[
            &["descriptionSnippet"],
            &["detailedMetadataSnippets", "...", "snippetText"],
        ],
    );

    let mut duration = renderer
        .get("lengthSeconds")
        .and_then(youtube_int_or_none)
        .map(|seconds| seconds as f64);
    if duration.is_none() {
        duration = youtube_renderer_text(
            renderer,
            &[
                &["lengthText"],
                &[
                    "thumbnailOverlays",
                    "...",
                    "thumbnailOverlayTimeStatusRenderer",
                    "text",
                ],
            ],
        )
        .and_then(|text| youtube_parse_duration(&text));
    }
    if duration.is_none() {
        let label = youtube_first_str(
            renderer,
            &[&["title", "accessibility", "accessibilityData", "label"]],
        )
        .unwrap_or_default();
        duration = youtube_duration_label_pattern()
            .and_then(|pattern| pattern.captures(&label).ok().flatten())
            .and_then(|captures| captures.name("duration"))
            .map(|part| part.as_str())
            .and_then(youtube_parse_duration);
    }

    let channel_id = youtube_first_str(
        renderer,
        &[&[
            "shortBylineText",
            "runs",
            "...",
            "navigationEndpoint",
            "browseEndpoint",
            "browseId",
        ]],
    )
    .or_else(|| {
        reel.and_then(|header| {
            youtube_first_str(
                header,
                &[&["channelNavigationEndpoint", "browseEndpoint", "browseId"]],
            )
        })
    });
    let channel_id = channel_id.as_deref().and_then(|id| youtube_ucid(Some(id)));

    let overlay_style = youtube_first_str(
        renderer,
        &[&[
            "thumbnailOverlays",
            "...",
            "thumbnailOverlayTimeStatusRenderer",
            "style",
        ]],
    );
    let badges = youtube_badges(renderer.get("badges").unwrap_or(&serde_json::Value::Null));
    let owner_badges = youtube_badges(
        renderer
            .get("ownerBadges")
            .unwrap_or(&serde_json::Value::Null),
    );
    let navigation_url = youtube_first_str(
        renderer,
        &[&[
            "navigationEndpoint",
            "commandMetadata",
            "webCommandMetadata",
            "url",
        ]],
    )
    .map(|path| youtube_join_url("https://www.youtube.com", &path))
    .unwrap_or_default();
    let url = if overlay_style.as_deref() == Some("SHORTS") || navigation_url.contains("/shorts/") {
        format!("https://www.youtube.com/shorts/{video_id}")
    } else {
        format!("https://www.youtube.com/watch?v={video_id}")
    };

    let time_text = youtube_renderer_text(renderer, &[&["publishedTimeText"], &["videoInfo"]])
        .or_else(|| reel.and_then(|header| youtube_renderer_text(header, &[&["timestampText"]])))
        .unwrap_or_default();
    let scheduled_timestamp = renderer
        .get("upcomingEventData")
        .and_then(|data| data.get("startTime"))
        .and_then(youtube_str_to_int);

    let live_status = if scheduled_timestamp.is_some() {
        Some("is_upcoming")
    } else if time_text.to_lowercase().contains("streamed") {
        Some("was_live")
    } else if overlay_style.as_deref() == Some("LIVE")
        || youtube_has_badge(&badges, YoutubeBadge::LiveNow)
    {
        Some("is_live")
    } else {
        None
    };

    let view_count_text = youtube_renderer_text(
        renderer,
        &[&["viewCountText"], &["shortViewCountText"], &["videoInfo"]],
    )
    .unwrap_or_default();
    // `videoInfo` is a string like '50K views - 10 years ago'.
    let view_count = if view_count_text.to_lowercase().contains("no views") {
        Some(0)
    } else {
        youtube_get_count(
            renderer,
            &[&["viewCountText"], &["shortViewCountText"], &["videoInfo"]],
        )
    };
    let view_count_field = if matches!(live_status, Some("is_live" | "is_upcoming")) {
        "concurrent_view_count"
    } else {
        "view_count"
    };

    let channel =
        youtube_renderer_text(renderer, &[&["ownerText"], &["shortBylineText"]]).or_else(|| {
            reel.and_then(|header| youtube_renderer_text(header, &[&["channelTitleText"]]))
        });

    let mut channel_handle = None;
    if let Some(runs) = renderer
        .get("shortBylineText")
        .and_then(|byline| byline.get("runs"))
        .and_then(serde_json::Value::as_array)
    {
        // Per-run alternation of the two handle paths under
        // `navigationEndpoint`, first hit wins.
        'runs: for run in runs {
            for tail in [
                &[
                    "navigationEndpoint",
                    "commandMetadata",
                    "webCommandMetadata",
                    "url",
                ][..],
                &["navigationEndpoint", "browseEndpoint", "canonicalBaseUrl"][..],
            ] {
                if let Some(handle) = youtube_first_str(run, &[tail])
                    .as_deref()
                    .and_then(|url| youtube_handle_from_url(Some(url)))
                {
                    channel_handle = Some(handle);
                    break 'runs;
                }
            }
        }
    }

    let availability = if youtube_has_badge(&badges, YoutubeBadge::AvailabilityPublic) {
        Some("public")
    } else {
        // The `or None` operands pass `True` or `None`, never `False`;
        // `needs_auth` is not passed, so `all_known` never fires here.
        let flag = |badge| youtube_has_badge(&badges, badge).then_some(true);
        youtube_availability(
            flag(YoutubeBadge::AvailabilityPrivate),
            flag(YoutubeBadge::AvailabilityPremium),
            flag(YoutubeBadge::AvailabilitySubscription),
            None,
            flag(YoutubeBadge::AvailabilityUnlisted),
        )
    };

    let mut entry = InfoDict::new();
    entry.insert("_type", serde_json::json!("url"));
    entry.insert("ie_key", serde_json::json!("Youtube"));
    entry.insert("id", serde_json::json!(video_id));
    entry.insert("url", serde_json::json!(url));
    entry.insert_if_some("title", title);
    entry.insert_if_some("description", description);
    if let Some(total) = duration {
        entry.insert("duration", youtube_duration_number(total));
    }
    entry.insert_if_some("channel_id", channel_id);
    entry.insert_if_some("channel", channel.clone());
    if let Some(channel_id) = entry.get("channel_id").and_then(serde_json::Value::as_str) {
        entry.insert(
            "channel_url",
            serde_json::json!(format!("https://www.youtube.com/channel/{channel_id}")),
        );
    }
    entry.insert_if_some("uploader", channel);
    entry.insert_if_some("uploader_id", channel_handle.clone());
    if let Some(handle) = channel_handle {
        entry.insert(
            "uploader_url",
            serde_json::json!(format!("https://www.youtube.com/{handle}")),
        );
    }
    entry.insert(
        "thumbnails",
        serde_json::Value::Array(youtube_entry_thumbnails(renderer)),
    );
    // `timestamp` mirrors the `approximate_date` gate: unset by default.
    entry.insert_if_some("release_timestamp", scheduled_timestamp);
    entry.insert_if_some("availability", availability);
    entry.insert_if_some(view_count_field, view_count);
    entry.insert_if_some("live_status", live_status);
    if youtube_has_badge(&owner_badges, YoutubeBadge::Verified) {
        entry.insert("channel_is_verified", serde_json::json!(true));
    }
    Some(entry)
}

/// Find the selected tab renderer, mirroring `_extract_selected_tab`. The
/// fatal error keeps the source message; the "please report this issue"
/// suffix is added by yt-dlp's error reporting, outside the extractor.
pub(crate) fn youtube_selected_tab(
    tabs: &[serde_json::Value],
    fatal: bool,
) -> Result<Option<&serde_json::Value>, ExtractorError> {
    if let Some(tab) = tabs
        .iter()
        .find(|tab| tab.get("selected").and_then(serde_json::Value::as_bool) == Some(true))
    {
        return Ok(Some(tab));
    }
    if fatal {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Unable to find selected tab",
        ));
    }
    Ok(None)
}

/// Resolve a tab renderer's id and lowercase name, mirroring
/// `_extract_tab_id_and_name`. A tab URL that does not parse falls through
/// to the `tabIdentifier`/name fallbacks instead of raising.
pub(crate) fn youtube_tab_id_and_name(tab: &serde_json::Value, base_url: &str) -> (String, String) {
    let tab_name = tab
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let tab_url = youtube_first_str(
        tab,
        &[&["endpoint", "commandMetadata", "webCommandMetadata", "url"]],
    )
    .map(|url| youtube_join_url(base_url, &url))
    .unwrap_or_default();
    let tab_id = (!tab_url.is_empty())
        .then(|| {
            youtube_tab_url_parts(&tab_url).map(|parts| {
                parts
                    .tab
                    .strip_prefix('/')
                    .unwrap_or(parts.tab.as_str())
                    .to_owned()
            })
        })
        .flatten()
        .filter(|id| !id.is_empty())
        .or_else(|| {
            tab.get("tabIdentifier")
                .and_then(serde_json::Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
        });
    if let Some(tab_id) = tab_id {
        let mapped = match tab_id.as_str() {
            "TAB_ID_SPONSORSHIPS" => "membership",
            _ => tab_id.as_str(),
        };
        return (mapped.to_owned(), tab_name);
    }
    // Fallback to the tab name when no id resolves.
    let mapped = match tab_name.as_str() {
        "home" => "featured",
        "live" => "streams",
        _ => tab_name.as_str(),
    };
    (mapped.to_owned(), tab_name)
}

/// Mirror `_has_tab`.
pub(crate) fn youtube_has_tab(tabs: &[serde_json::Value], tab_id: &str) -> bool {
    tabs.iter()
        .any(|tab| youtube_tab_id_and_name(tab, "https://www.youtube.com").0 == tab_id)
}

/// Mirror `_video_entry`: the `videoId` gate in front of `_extract_video`.
pub(crate) fn youtube_tab_video_entry(renderer: &serde_json::Value) -> Option<InfoDict> {
    renderer
        .get("videoId")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())?;
    youtube_extract_video(renderer)
}

/// Unwrap one grid item to its basic renderer, mirroring
/// `_extract_basic_item_renderer`.
fn youtube_basic_item_renderer<'v>(item: &'v serde_json::Value) -> Option<&'v serde_json::Value> {
    let map = item.as_object()?;
    for (key, renderer) in map {
        if !renderer.is_object() {
            continue;
        }
        if matches!(
            key.as_str(),
            "playlistRenderer"
                | "videoRenderer"
                | "channelRenderer"
                | "showRenderer"
                | "reelItemRenderer"
        ) || (key.starts_with("grid") && key.ends_with("Renderer"))
        {
            return Some(renderer);
        }
    }
    None
}

/// Extract one grid item's video or playlist entry, mirroring the video
/// and playlist branches of `_grid_entries`. Channel renderers and generic
/// endpoint URLs stay TODOs.
fn youtube_grid_item_entry(item: &serde_json::Value) -> Option<InfoDict> {
    let renderer = youtube_basic_item_renderer(item)?;
    if renderer.get("playlistId").is_some() {
        return youtube_tab_playlist_entry(renderer);
    }
    if renderer
        .get("videoId")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|id| !id.is_empty())
    {
        return youtube_extract_video(renderer);
    }
    // TODO: `_extract_channel_renderer` and generic endpoint dispatch.
    None
}

/// Build a tab `url_result` for a playlist renderer, mirroring the
/// playlist branches of `_rich_entries` and `_grid_entries`
/// (`ie_key` `YoutubeTab`).
fn youtube_tab_playlist_entry(renderer: &serde_json::Value) -> Option<InfoDict> {
    let playlist_id = renderer
        .get("playlistId")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())?;
    Some(youtube_url_entry(
        &format!("https://www.youtube.com/playlist?list={playlist_id}"),
        "YoutubeTab",
        Some(playlist_id),
        youtube_renderer_text(renderer, &[&["title"]]),
    ))
}

/// Mirror the video and playlist branches of `_rich_entries`. Lockup view
/// models (playlists tab) and `shortsLockupViewModel` extraction stay TODOs.
fn youtube_rich_entries(item: &serde_json::Value) -> Vec<InfoDict> {
    let Some(content) = item
        .get("richItemRenderer")
        .and_then(|item| item.get("content"))
        .filter(|content| content.is_object())
    else {
        return Vec::new();
    };
    if content.get("lockupViewModel").is_some() {
        // TODO: `_extract_lockup_view_model` (playlists-tab lockups).
        return Vec::new();
    }
    let renderer = ["videoRenderer", "reelItemRenderer", "playlistRenderer"]
        .iter()
        .find_map(|key| content.get(*key));
    // TODO: `shortsLockupViewModel` extraction (shorts tab).
    let Some(renderer) = renderer.filter(|renderer| renderer.is_object()) else {
        return Vec::new();
    };
    if renderer.get("videoId").is_some() {
        return youtube_tab_video_entry(renderer).into_iter().collect();
    }
    youtube_tab_playlist_entry(renderer).into_iter().collect()
}

/// Extract one page of tab entries plus the next continuation query,
/// mirroring `_extract_entries` for the Videos-tab shapes
/// (`itemSectionRenderer` with `videoRenderer` children,
/// `richGridRenderer` recursion, and top-level `richItemRenderer` items).
/// Grid/shelf/music/post/playlist/channel/hashtag/lockup children keep
/// their first-known-key-wins control flow but contribute no entries yet;
/// their continuations are still followed. Report-history sections stay
/// TODO. Key order follows `serde_json::Map` (sorted; the workspace does
/// not enable `preserve_order`), which matches source order for the
/// single-key items renderers emit in practice.
pub(crate) fn youtube_extract_tab_contents(
    parent: &serde_json::Value,
) -> (Vec<InfoDict>, Option<serde_json::Value>) {
    let mut entries = Vec::new();
    let mut continuation = None;
    let contents = parent
        .get("contents")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for content in contents {
        if !content.is_object() {
            continue;
        }
        let is_renderer = [
            "itemSectionRenderer",
            "musicShelfRenderer",
            "musicShelfContinuation",
        ]
        .iter()
        .find_map(|key| content.get(*key))
        .filter(|renderer| renderer.is_object());
        let Some(is_renderer) = is_renderer else {
            if content.get("richItemRenderer").is_some() {
                entries.extend(youtube_rich_entries(content));
                continuation = youtube_extract_continuation(parent);
            }
            // TODO: `reportHistorySectionRenderer` (report-history page).
            continue;
        };
        let isr_contents = is_renderer
            .get("contents")
            .and_then(serde_json::Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for isr_content in isr_contents {
            let serde_json::Value::Object(map) = isr_content else {
                continue;
            };
            // First known key in item order wins, mirroring the source loop.
            for (key, renderer) in map {
                match key.as_str() {
                    "playlistVideoListRenderer" if renderer.is_object() => {
                        entries.extend(youtube_playlist_list_entries(renderer));
                        continuation = youtube_extract_continuation(renderer);
                        break;
                    }
                    "videoRenderer" if renderer.is_object() => {
                        entries.extend(youtube_tab_video_entry(renderer));
                        continuation = youtube_extract_continuation(renderer);
                        break;
                    }
                    "richGridRenderer" if renderer.is_object() => {
                        // The source yields the sub-generator here; the port
                        // flattens it so entries stay entries.
                        let (sub_entries, sub_continuation) =
                            youtube_extract_tab_contents(renderer);
                        entries.extend(sub_entries);
                        continuation = sub_continuation;
                        break;
                    }
                    "gridRenderer"
                    | "reelShelfRenderer"
                    | "shelfRenderer"
                    | "musicResponsiveListItemRenderer"
                    | "backstagePostThreadRenderer"
                    | "playlistRenderer"
                    | "channelRenderer"
                    | "hashtagTileRenderer"
                    | "lockupViewModel" => {
                        // TODO: grid/shelf/music/post/playlist/channel/
                        // hashtag/lockup entries. The continuation still
                        // follows this renderer, as in the source.
                        continuation = youtube_extract_continuation(renderer);
                        break;
                    }
                    _ => continue,
                }
            }
        }
        if continuation.is_none() {
            continuation = youtube_extract_continuation(is_renderer);
        }
    }
    if continuation.is_none() {
        continuation = youtube_extract_continuation(parent);
    }
    (entries, continuation)
}

/// Read the first page of a tab's content, mirroring the parent selection
/// of `_entries` (`sectionListRenderer`, then `richGridRenderer`).
pub(crate) fn youtube_tab_first_page(
    tab: &serde_json::Value,
) -> (Vec<InfoDict>, Option<serde_json::Value>) {
    let parent = tab
        .get("content")
        .and_then(|content| {
            content
                .get("sectionListRenderer")
                .or_else(|| content.get("richGridRenderer"))
                .filter(|renderer| renderer.is_object())
        })
        .unwrap_or(&serde_json::Value::Null);
    youtube_extract_tab_contents(parent)
}

fn youtube_relative_time_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    // Mirrors `extract_relative_time`: unit alternation longest-first so
    // `second` wins over `s`, case-sensitive like the source.
    PATTERN.get_or_init(|| {
        regex::Regex::new(
            r"(?P<start>today|yesterday|now)|(?P<time>\d+)\s*(?P<unit>second|minute|month|hour|week|year|sec|min|day|hr|wk|mo|yr|s|h|d|w|y)s?\s*ago",
        )
        .expect("static relative-time pattern")
    })
}

fn youtube_now_epoch() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs_f64())
        .unwrap_or(0.0)
}

fn youtube_days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        // Unreachable for civil dates; keeps month math total.
        _ => 30,
    }
}

fn youtube_days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 {
        year / 400
    } else {
        (year - 399) / 400
    };
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn youtube_civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted / 146_097
    } else {
        (shifted - 146_096) / 146_097
    };
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month, day)
}

/// Add whole months to an epoch timestamp, mirroring
/// `datetime_add_months` (day clamped to the target month, time of day
/// kept).
fn youtube_add_months(epoch: f64, months: i64) -> f64 {
    let days = epoch.div_euclid(86_400.0);
    let time_of_day = epoch - days * 86_400.0;
    let (mut year, mut month, day) = youtube_civil_from_days(days as i64);
    let shifted = month - 1 + months;
    year += shifted.div_euclid(12);
    month = shifted.rem_euclid(12) + 1;
    let day = day.min(youtube_days_in_month(year, month));
    youtube_days_from_civil(year, month, day) as f64 * 86_400.0 + time_of_day
}

/// Mirror `datetime_round`'s half-up float rounding to whole units.
fn youtube_round_to_unit(epoch: f64, unit_secs: f64) -> f64 {
    ((epoch + unit_secs / 2.0) / unit_secs).floor() * unit_secs
}

/// Relative branch of `_parse_time_text`/`extract_relative_time` as a Unix
/// timestamp. Absolute dates need `unified_timestamp` (TODO) and stay
/// `None`, as do absurd magnitudes (the source raises `OverflowError`
/// there instead).
pub(crate) fn youtube_parse_time_text(text: &str) -> Option<i64> {
    let captures = youtube_relative_time_pattern().captures(text)?;
    if let Some(start) = captures.name("start").map(|part| part.as_str()) {
        let now = youtube_now_epoch();
        // `today`/`now` keep the time of day; only `yesterday` shifts.
        return Some(
            if start == "yesterday" {
                now - 86_400.0
            } else {
                now
            }
            .floor() as i64,
        );
    }
    let amount: f64 = captures.name("time")?.as_str().parse().ok()?;
    let now = youtube_now_epoch();
    let (epoch, precision) = match captures.name("unit")?.as_str() {
        "second" | "sec" | "s" => (now - amount, 1.0),
        "minute" | "min" => (now - amount * 60.0, 60.0),
        "hour" | "hr" | "h" => (now - amount * 3_600.0, 3_600.0),
        "day" | "d" => (now - amount * 86_400.0, 86_400.0),
        // Weeks round to days, months and years add calendar months.
        "week" | "wk" | "w" => (now - amount * 604_800.0, 86_400.0),
        "month" | "mo" => (youtube_add_months(now, -(amount as i64)), 86_400.0),
        "year" | "yr" | "y" => (youtube_add_months(now, -(amount as i64) * 12), 86_400.0),
        _ => return None,
    };
    Some(youtube_round_to_unit(epoch, precision) as i64)
}

/// Mirror `strftime_or_none(timestamp)` (`%Y%m%d`, UTC).
pub(crate) fn youtube_upload_date(timestamp: i64) -> Option<String> {
    let (year, month, day) = youtube_civil_from_days(timestamp.div_euclid(86_400));
    (0..10_000)
        .contains(&year)
        .then(|| format!("{year:04}{month:02}{day:02}"))
}

/// POSIX-subset `shlex.split` for renderer keyword strings: whitespace
/// separation with single/double quotes; a backslash escapes the next
/// character and an unterminated quote consumes the rest.
pub(crate) fn youtube_split_words(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote = None;
    let mut chars = text.chars();
    while let Some(char) = chars.next() {
        if let Some(open) = quote {
            if char == open {
                quote = None;
            } else if char == '\\' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
                in_token = true;
            } else {
                current.push(char);
                in_token = true;
            }
        } else if char == '\'' || char == '"' {
            quote = Some(char);
            in_token = true;
        } else if char == '\\' {
            if let Some(next) = chars.next() {
                current.push(next);
            }
            in_token = true;
        } else if char.is_whitespace() {
            if in_token {
                tokens.push(std::mem::take(&mut current));
                in_token = false;
            }
        } else {
            current.push(char);
            in_token = true;
        }
    }
    if in_token {
        tokens.push(current);
    }
    tokens
}

fn youtube_ucid_from_url_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"^(?:https?://(?:www\.)?youtube\.com)?/(UC[\w-]{22})")
            .expect("static ucid-from-url pattern")
    })
}

/// Mirror `ucid_from_url` (no unquoting, unlike the handle helpers).
pub(crate) fn youtube_ucid_from_url(url: Option<&str>) -> Option<String> {
    youtube_ucid_from_url_pattern()
        .captures(url.unwrap_or(""))
        .and_then(|captures| captures.get(1))
        .map(|ucid| ucid.as_str().to_owned())
}

fn youtube_handle_or_none_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"^(@[\w.-]{3,30})$").expect("static handle-or-none pattern")
    })
}

/// Mirror `handle_or_none`: full match on the unquoted handle.
pub(crate) fn youtube_handle_or_none(handle: Option<&str>) -> Option<String> {
    let decoded = youtube_percent_decode(handle.unwrap_or(""));
    youtube_handle_or_none_pattern()
        .is_match(&decoded)
        .then(|| decoded)
}

/// Mirror truthiness for sidebar renderers.
fn youtube_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::Number(number) => {
            number.as_i64() != Some(0) && number.as_f64() != Some(0.0)
        }
        serde_json::Value::String(text) => !text.is_empty(),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(map) => !map.is_empty(),
    }
}

/// Find a sidebar info renderer by plain key, mirroring
/// `_extract_sidebar_info_renderer`.
pub(crate) fn youtube_sidebar_info_renderer<'v>(
    data: &'v serde_json::Value,
    key: &str,
) -> Option<&'v serde_json::Value> {
    data.get("sidebar")?
        .get("playlistSidebarRenderer")?
        .get("items")?
        .as_array()?
        .iter()
        .filter_map(|item| item.get(key))
        .find(|renderer| renderer.is_object() && youtube_truthy(renderer))
}

fn youtube_privacy_dropdown_icon(
    header: &serde_json::Value,
    sidebar: &serde_json::Value,
) -> Option<String> {
    for scope in [header, sidebar] {
        let entries = scope
            .get("privacyForm")
            .and_then(|form| form.get("dropdownFormFieldRenderer"))
            .and_then(|field| field.get("dropdown"))
            .and_then(|dropdown| dropdown.get("dropdownRenderer"))
            .and_then(|renderer| renderer.get("entries"))
            .and_then(serde_json::Value::as_array);
        for entry in entries.into_iter().flatten() {
            let item = entry.get("privacyDropdownItemRenderer");
            let selected = item
                .and_then(|item| item.get("isSelected"))
                .and_then(serde_json::Value::as_bool)
                == Some(true);
            if !selected {
                continue;
            }
            if let Some(icon) = item
                .and_then(|item| item.get("icon"))
                .and_then(|icon| icon.get("iconType"))
                .and_then(serde_json::Value::as_str)
            {
                return Some(icon.to_owned());
            }
        }
    }
    None
}

/// Mirror `_extract_availability`. Sidebar badges are always empty here:
/// the source passes a tuple key into `_extract_sidebar_info_renderer`,
/// whose `try_get` swallows the resulting `TypeError` (pinned by oracle:
/// a sidebar `PRIVACY_PRIVATE` badge still yields `None`).
pub(crate) fn youtube_tab_availability(data: &serde_json::Value) -> Option<String> {
    let header = data
        .get("header")
        .and_then(|header| header.get("playlistHeaderRenderer"))
        .filter(|header| header.is_object())
        .unwrap_or(&serde_json::Value::Null);
    let player_privacy = header.get("privacy").and_then(serde_json::Value::as_str);
    let setting_icon = youtube_privacy_dropdown_icon(header, &serde_json::Value::Null);
    let microformat = data
        .get("microformat")
        .and_then(|microformat| microformat.get("microformatDataRenderer"))
        .filter(|renderer| renderer.is_object());
    let is_private = match player_privacy {
        Some(privacy) => Some(privacy == "PRIVATE"),
        None => setting_icon
            .as_deref()
            .map(|icon| icon == "PRIVACY_PRIVATE"),
    };
    let is_unlisted = match player_privacy {
        Some(privacy) => Some(privacy == "UNLISTED"),
        None => match setting_icon.as_deref() {
            Some(icon) => Some(icon == "PRIVACY_UNLISTED"),
            None => microformat
                .and_then(|renderer| renderer.get("unlisted"))
                .and_then(serde_json::Value::as_bool),
        },
    };
    if let Some(availability) =
        youtube_availability(is_private, None, None, Some(false), is_unlisted)
    {
        return Some(availability.to_owned());
    }
    if player_privacy == Some("PUBLIC")
        || setting_icon.as_deref() == Some("PRIVACY_PUBLIC")
        || microformat
            .and_then(|renderer| renderer.get("noindex"))
            .and_then(serde_json::Value::as_bool)
            == Some(false)
    {
        return Some("public".to_owned());
    }
    None
}

/// Read one indexed text of a stats list, mirroring `_get_text(stats, i)`.
fn youtube_stats_text(stats: &serde_json::Value, index: usize) -> Option<String> {
    youtube_textish(stats.as_array()?.get(index)?)
}

/// Read one indexed count of a stats list, mirroring `_get_count(stats, i)`.
fn youtube_stats_count(stats: &serde_json::Value, index: usize) -> Option<i64> {
    youtube_count_text_value(&youtube_stats_text(stats, index).unwrap_or_default())
}

/// Read one `playlistBylineRenderer` text or count under a playlist header.
fn youtube_byline_text(header: &serde_json::Value, index: usize) -> Option<String> {
    let item = header.get("byline")?.as_array()?.get(index)?;
    youtube_textish(item.get("playlistBylineRenderer")?.get("text")?)
}

fn youtube_byline_count(header: &serde_json::Value, index: usize) -> Option<i64> {
    youtube_count_text_value(&youtube_byline_text(header, index).unwrap_or_default())
}

/// First present `stats`/`briefStats`/`numVideosText` node over the primary
/// sidebar and playlist header renderers, mirroring `get_first`.
fn youtube_stats_renderer<'v>(
    sidebar: Option<&'v serde_json::Value>,
    header: Option<&'v serde_json::Value>,
) -> Option<&'v serde_json::Value> {
    for scope in [sidebar, header].into_iter().flatten() {
        for key in ["stats", "briefStats", "numVideosText"] {
            if let Some(stats) = scope.get(key) {
                return Some(stats);
            }
        }
    }
    None
}

fn youtube_owner_channel_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"^by (.+) and \d+ others?$").expect("static owner pattern")
    })
}

/// Mirror `_extract_metadata_from_tabs`.
///
/// `modified_date` only resolves for relative update texts: absolute dates
/// need `unified_timestamp` (TODO) and stay unset, matching the `timestamp`
/// treatment in `youtube_extract_video`.
pub(crate) fn youtube_tab_metadata(item_id: &str, data: &serde_json::Value) -> InfoDict {
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(item_id));

    let channel_meta = data
        .get("metadata")
        .and_then(|metadata| metadata.get("channelMetadataRenderer"))
        .filter(|renderer| renderer.is_object());
    if let Some(meta) = channel_meta {
        let channel_id = meta
            .get("externalId")
            .and_then(serde_json::Value::as_str)
            .and_then(|id| youtube_ucid(Some(id)))
            .or_else(|| {
                meta.get("channelUrl")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|url| youtube_ucid_from_url(Some(url)))
            });
        info.insert_if_some(
            "channel",
            meta.get("title")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        );
        info.insert_if_some("channel_id", channel_id.clone());
        if channel_id.is_some() {
            info.insert("id", serde_json::json!(channel_id));
        }
    }
    let playlist_meta = if channel_meta.is_none() {
        data.get("metadata")
            .and_then(|metadata| metadata.get("playlistMetadataRenderer"))
            .filter(|renderer| renderer.is_object())
    } else {
        None
    };
    let meta_renderer = channel_meta.or(playlist_meta);

    let page_header = data
        .get("header")
        .and_then(|header| header.get("pageHeaderRenderer"))
        .and_then(|header| header.get("content"))
        .and_then(|content| content.get("pageHeaderViewModel"))
        .filter(|view| view.is_object())
        .unwrap_or(&serde_json::Value::Null);

    // Uncropped avatar: the first avatar URL with its crop params replaced.
    let mut avatar_thumbnails = channel_meta
        .map(|meta| youtube_thumbnails_at(meta, &["avatar"], "thumbnails"))
        .unwrap_or_default();
    if let Some(url) = avatar_thumbnails
        .first()
        .and_then(|thumbnail| thumbnail.get("url"))
        .and_then(serde_json::Value::as_str)
    {
        let uncropped = format!("{}=s0", url.split('=').next().unwrap_or(url));
        if youtube_url_str_or_none(&uncropped).is_some() {
            avatar_thumbnails.push(serde_json::json!({
                "url": uncropped,
                "id": "avatar_uncropped",
                "preference": 1,
            }));
        }
    }

    let mut channel_banners = Vec::new();
    for key in ["banner", "mobileBanner", "tvBanner"] {
        channel_banners.extend(youtube_thumbnails_at(
            data,
            &["header", "...", key],
            "thumbnails",
        ));
    }
    channel_banners.extend(youtube_thumbnails_at(
        page_header,
        &["banner", "imageBannerViewModel", "image"],
        "sources",
    ));
    for banner in &mut channel_banners {
        if let Some(banner) = banner.as_object_mut() {
            banner.insert("preference".to_owned(), serde_json::json!(-10));
        }
    }
    if let Some(url) = channel_banners
        .first()
        .and_then(|banner| banner.get("url"))
        .and_then(serde_json::Value::as_str)
    {
        let uncropped = format!("{}=s0", url.split('=').next().unwrap_or(url));
        if youtube_url_str_or_none(&uncropped).is_some() {
            channel_banners.push(serde_json::json!({
                "url": uncropped,
                "id": "banner_uncropped",
                "preference": -5,
            }));
        }
    }

    let sidebar_primary = youtube_sidebar_info_renderer(data, "playlistSidebarPrimaryInfoRenderer");
    let playlist_header = data
        .get("header")
        .and_then(|header| header.get("playlistHeaderRenderer"))
        .filter(|header| header.is_object());

    let mut primary_thumbnails = Vec::new();
    if let Some(sidebar) = sidebar_primary {
        for key in [
            "playlistVideoThumbnailRenderer",
            "playlistCustomThumbnailRenderer",
        ] {
            primary_thumbnails.extend(youtube_thumbnails_at(
                sidebar,
                &["thumbnailRenderer", key, "thumbnail"],
                "thumbnails",
            ));
        }
    }
    let mut playlist_thumbnails = Vec::new();
    if let Some(header) = playlist_header {
        playlist_thumbnails.extend(youtube_thumbnails_at(
            header,
            &[
                "playlistHeaderBanner",
                "heroPlaylistThumbnailRenderer",
                "thumbnail",
            ],
            "thumbnails",
        ));
    }
    let mut thumbnails = if primary_thumbnails.is_empty() {
        playlist_thumbnails
    } else {
        primary_thumbnails
    };
    thumbnails.extend(avatar_thumbnails);
    thumbnails.extend(channel_banners);

    let title = meta_renderer
        .and_then(|meta| meta.get("title"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .or_else(|| youtube_renderer_text(data, &[&["header", "hashtagHeaderRenderer", "hashtag"]]))
        .unwrap_or_else(|| info.get_str("id").unwrap_or(item_id).to_owned());
    let description = meta_renderer.map(|meta| {
        meta.get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned()
    });
    let mut tags: Vec<String> = youtube_traverse_nodes(
        data,
        &["microformat", "microformatDataRenderer", "tags", "..."],
    )
    .iter()
    .filter_map(|tag| tag.as_str())
    .map(str::to_owned)
    .collect();
    if tags.is_empty() {
        tags = meta_renderer
            .and_then(|meta| meta.get("keywords"))
            .and_then(serde_json::Value::as_str)
            .map(youtube_split_words)
            .unwrap_or_default();
    }

    info.insert("title", serde_json::json!(title));
    info.insert_if_some("availability", youtube_tab_availability(data));
    let follower_count = youtube_get_count(data, &[&["header", "...", "subscriberCountText"]]);
    let follower_count = match follower_count {
        // A zero count falls through to the page-header rows, like `or`.
        Some(count) if count != 0 => Some(count),
        _ => {
            let rows = page_header
                .get("metadata")
                .and_then(|metadata| metadata.get("contentMetadataViewModel"))
                .and_then(|view| view.get("metadataRows"))
                .and_then(serde_json::Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let mut found = None;
            for part in rows
                .iter()
                .filter_map(|row| row.get("metadataParts"))
                .filter_map(serde_json::Value::as_array)
                .flatten()
            {
                let content = part
                    .get("text")
                    .and_then(|text| text.get("content"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                if content.contains("subscribers") {
                    if let Some(count) = youtube_parse_count(content) {
                        found = Some(count);
                        break;
                    }
                }
            }
            found
        }
    };
    info.insert_if_some("channel_follower_count", follower_count);
    info.insert_if_some("description", description);
    info.insert(
        "tags",
        serde_json::Value::Array(tags.into_iter().map(serde_json::Value::from).collect()),
    );
    info.insert("thumbnails", serde_json::Value::Array(thumbnails));

    let channel_handle = channel_meta
        .and_then(|meta| meta.get("vanityChannelUrl"))
        .and_then(serde_json::Value::as_str)
        .and_then(|url| youtube_handle_from_url(Some(url)))
        .or_else(|| {
            channel_meta
                .and_then(|meta| meta.get("ownerUrls"))
                .and_then(serde_json::Value::as_array)
                .and_then(|urls| {
                    urls.iter()
                        .filter_map(serde_json::Value::as_str)
                        .find_map(|url| youtube_handle_from_url(Some(url)))
                })
        })
        .or_else(|| {
            youtube_traverse_first(data, &[&["header", "...", "channelHandleText"]])
                .and_then(serde_json::Value::as_str)
                .and_then(|text| youtube_handle_or_none(Some(text)))
        });
    if let Some(handle) = &channel_handle {
        info.insert("uploader_id", serde_json::json!(handle));
        info.insert(
            "uploader_url",
            serde_json::json!(format!("https://www.youtube.com/{handle}")),
        );
    }

    let channel_badges = youtube_badges(
        youtube_traverse_first(data, &[&["header", "...", "badges"]])
            .unwrap_or(&serde_json::Value::Null),
    );
    let page_header_marks: Vec<&str> = youtube_traverse_nodes(
        page_header,
        &[
            "title",
            "dynamicTextViewModel",
            "text",
            "attachmentRuns",
            "...",
            "element",
            "type",
            "imageType",
            "image",
            "sources",
            "...",
            "clientResource",
            "imageName",
        ],
    )
    .into_iter()
    .filter_map(serde_json::Value::as_str)
    .collect();
    if youtube_has_badge(&channel_badges, YoutubeBadge::Verified)
        || page_header_marks.contains(&"CHECK_CIRCLE_FILLED")
        || page_header_marks.contains(&"AUDIO_BADGE")
    {
        info.insert("channel_is_verified", serde_json::json!(true));
    }

    let stats = youtube_stats_renderer(sidebar_primary, playlist_header);
    let last_updated = stats
        .and_then(|stats| youtube_stats_text(stats, 2))
        .or_else(|| {
            playlist_header.and_then(|header| {
                header
                    .get("byline")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|byline| byline.get(1))
                    .and_then(|item| item.get("playlistBylineRenderer"))
                    .and_then(|renderer| renderer.get("text"))
                    .and_then(youtube_textish)
            })
        });
    info.insert_if_some(
        "modified_date",
        last_updated
            .as_deref()
            .and_then(youtube_parse_time_text)
            .and_then(youtube_upload_date),
    );

    let view_count = stats
        .and_then(|stats| youtube_stats_count(stats, 1))
        .or_else(|| {
            playlist_header.and_then(|header| {
                youtube_count_text_value(
                    &youtube_textish(header.get("viewCountText")?).unwrap_or_default(),
                )
            })
        })
        .or_else(|| {
            youtube_get_count(
                data,
                &[&[
                    "contents",
                    "twoColumnBrowseResultsRenderer",
                    "tabs",
                    "...",
                    "tabRenderer",
                    "content",
                    "sectionListRenderer",
                    "contents",
                    "...",
                    "itemSectionRenderer",
                    "contents",
                    "...",
                    "channelAboutFullMetadataRenderer",
                    "viewCountText",
                ]],
            )
        });
    info.insert_if_some("view_count", view_count);

    let playlist_count = stats
        .and_then(|stats| youtube_stats_count(stats, 0))
        .or_else(|| playlist_header.and_then(|header| youtube_byline_count(header, 0)));
    info.insert_if_some("playlist_count", playlist_count);

    if info.get("channel_id").is_none() {
        let owner = playlist_header
            .and_then(|header| header.get("ownerText"))
            .filter(|owner| youtube_truthy(owner))
            .or_else(|| {
                youtube_sidebar_info_renderer(data, "playlistSidebarSecondaryInfoRenderer")
                    .and_then(|sidebar| sidebar.get("videoOwner"))
                    .and_then(|owner| owner.get("videoOwnerRenderer"))
                    .and_then(|renderer| renderer.get("title"))
            });
        let owner_text = owner.and_then(youtube_textish);
        let browse_ep = owner
            .and_then(|owner| owner.get("runs"))
            .and_then(serde_json::Value::as_array)
            .and_then(|runs| runs.first())
            .and_then(|run| run.get("navigationEndpoint"))
            .and_then(|endpoint| endpoint.get("browseEndpoint"));
        let channel = owner_text.as_deref().and_then(|text| {
            youtube_owner_channel_pattern()
                .captures(text)
                .and_then(|captures| captures.get(1))
                .map(|owner| owner.as_str().to_owned())
                .or_else(|| owner_text.clone())
        });
        info.insert_if_some("channel", channel);
        info.insert_if_some(
            "channel_id",
            browse_ep
                .and_then(|endpoint| endpoint.get("browseId"))
                .and_then(serde_json::Value::as_str)
                .and_then(|id| youtube_ucid(Some(id))),
        );
        info.insert_if_some(
            "uploader_id",
            browse_ep
                .and_then(|endpoint| endpoint.get("canonicalBaseUrl"))
                .and_then(serde_json::Value::as_str)
                .and_then(|base| {
                    youtube_handle_from_url(Some(&youtube_join_url(
                        "https://www.youtube.com",
                        base,
                    )))
                }),
        );
    }

    let uploader = info
        .get("channel")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    info.insert_if_some("uploader", uploader);
    if let Some(channel_id) = info.get_str("channel_id") {
        info.insert(
            "channel_url",
            serde_json::json!(format!("https://www.youtube.com/channel/{channel_id}")),
        );
    }
    if let Some(uploader_id) = info.get_str("uploader_id") {
        info.insert(
            "uploader_url",
            serde_json::json!(format!("https://www.youtube.com/{uploader_id}")),
        );
    }
    info
}

/// Read one browse-response page the way the `_entries` continuation loop
/// does: entries for the known continuation shapes plus the next query.
/// Video/playlist entries flow through the `_grid_entries`,
/// `_playlist_entries`, and `_extract_entries` shapes; channel renderers,
/// generic endpoint URLs, lockups, posts, and report rows stay TODO but
/// their continuations are still followed. Across several known keys the
/// first continuation wins (the source effectively keeps the last write;
/// real responses carry one arm). The `playlistVideoListContinuation`,
/// `gridContinuation`, and `itemSectionContinuation` wrappers that the
/// source only ever nests (and that raise `TypeError` as bare first items)
/// degrade gracefully to their own continuation instead of raising.
pub(crate) fn youtube_tab_continuation_entries(
    response: &serde_json::Value,
) -> (Vec<InfoDict>, Option<serde_json::Value>) {
    let items = ["onResponseReceivedActions", "onResponseReceivedEndpoints"]
        .iter()
        .filter_map(|key| response.get(*key))
        .filter_map(serde_json::Value::as_array)
        .flat_map(|actions| actions.iter())
        .filter_map(|action| action.get("appendContinuationItemsAction"))
        .filter_map(|action| action.get("continuationItems"))
        .next()
        .or_else(|| response.get("continuationContents"));
    // `traverse_obj(items, 0, None, expected_type=dict, default={})`: the
    // first item when it is an object, else an empty object.
    let item = match items {
        Some(serde_json::Value::Array(items)) => items
            .first()
            .filter(|item| item.is_object())
            .unwrap_or(&serde_json::Value::Null),
        Some(item @ serde_json::Value::Object(_)) => item,
        _ => &serde_json::Value::Null,
    };
    let mut entries = Vec::new();
    let mut continuation = None;
    let serde_json::Value::Object(map) = item else {
        return (entries, None);
    };
    // Every known key is dispatched (no first-wins break here, unlike
    // `_extract_entries`).
    for (key, _) in map {
        match key.as_str() {
            "videoRenderer"
            | "gridPlaylistRenderer"
            | "gridVideoRenderer"
            | "gridChannelRenderer" => {
                // `_grid_entries` over every continuation item.
                if let Some(items) = items.and_then(serde_json::Value::as_array) {
                    for item in items {
                        entries.extend(youtube_grid_item_entry(item));
                    }
                }
                let wrapped = serde_json::json!({"items": items});
                continuation = continuation.or_else(|| youtube_extract_continuation(&wrapped));
            }
            "playlistVideoRenderer" => {
                let wrapped = serde_json::json!({"contents": items});
                entries.extend(youtube_playlist_list_entries(&wrapped));
                continuation = continuation.or_else(|| youtube_extract_continuation(&wrapped));
            }
            "itemSectionRenderer" | "richItemRenderer" => {
                let wrapped = serde_json::json!({"contents": items});
                let (sub_entries, sub_continuation) = youtube_extract_tab_contents(&wrapped);
                entries.extend(sub_entries);
                continuation = continuation
                    .or(sub_continuation)
                    .or_else(|| youtube_extract_continuation(&wrapped));
            }
            "playlistVideoListContinuation" | "gridContinuation" | "itemSectionContinuation" => {
                // The source raises `TypeError` on these bare wrappers;
                // follow the wrapper's own continuation instead.
                continuation = continuation.or_else(|| youtube_extract_continuation(item));
            }
            "backstagePostThreadRenderer" | "reportHistoryTableRowRenderer" => {
                // TODO: post/report continuation entries.
                let wrapped = serde_json::json!({"contents": items});
                continuation = continuation.or_else(|| youtube_extract_continuation(&wrapped));
            }
            "sectionListContinuation" => {
                // `extract_entries` over the bare list yields nothing but is
                // graceful in the source too.
                let wrapped = items.cloned().unwrap_or(serde_json::Value::Null);
                let (sub_entries, sub_continuation) = youtube_extract_tab_contents(&wrapped);
                entries.extend(sub_entries);
                continuation = continuation
                    .or(sub_continuation)
                    .or_else(|| youtube_extract_continuation(&wrapped));
            }
            "lockupViewModel" => {
                // TODO: `_grid_entries` lockup extraction.
                let wrapped = serde_json::json!({"items": items});
                continuation = continuation.or_else(|| youtube_extract_continuation(&wrapped));
            }
            _ => continue,
        }
    }
    if continuation.is_none() {
        let single = serde_json::json!({"contents": [item]});
        continuation = youtube_extract_continuation(&single);
    }
    (entries, continuation)
}

use std::collections::HashSet;

/// Follow tab continuations, mirroring the `_entries` browse loop. The
/// caller supplies page fetching (`query` and 1-based page number to an
/// optional browse-response JSON); loop detection, paging, and the empty
/// response break match the source.
pub(crate) fn youtube_collect_tab_entries(
    first_page: (Vec<InfoDict>, Option<serde_json::Value>),
    fetch: &mut dyn FnMut(&serde_json::Value, u32) -> Option<serde_json::Value>,
) -> Vec<InfoDict> {
    let (mut entries, mut continuation) = first_page;
    let mut seen = HashSet::new();
    let mut page_num = 1u32;
    while let Some(query) = continuation.take() {
        let token = query
            .get("continuation")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        if let Some(token) = &token {
            if !seen.insert(token.clone()) {
                // Looping feed: same token twice ends pagination.
                break;
            }
        }
        let Some(response) = fetch(&query, page_num) else {
            break;
        };
        page_num += 1;
        let (mut page_entries, next) = youtube_tab_continuation_entries(&response);
        entries.append(&mut page_entries);
        continuation = next;
    }
    entries
}

/// Compose a tab playlist from initial data, mirroring `_extract_from_tabs`
/// for the first page: metadata, the selected-tab title suffix, and
/// first-page entries. Browse-API pagination stays TODO.
pub(crate) fn youtube_tab_playlist(
    item_id: &str,
    data: &serde_json::Value,
    tabs: &[serde_json::Value],
) -> Result<ExtractorResult, ExtractorError> {
    let mut info = youtube_tab_metadata(item_id, data);
    let Some(selected) = youtube_selected_tab(tabs, true)? else {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Unable to find selected tab",
        ));
    };
    let mut title = info.get_str("title").unwrap_or(item_id).to_owned();
    for key in ["title", "expandedText"] {
        if let Some(text) = selected
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.is_empty())
        {
            title.push_str(" - ");
            title.push_str(text);
        }
    }
    info.insert("title", serde_json::json!(title));
    let (entries, _continuation) = youtube_tab_first_page(selected);
    Ok(ExtractorResult::Playlist { info, entries })
}
