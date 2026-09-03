/// Native port of yt-dlp's `FormatSorter` (`yt_dlp/utils/_utils.py`).
///
/// Formats are ranked worst-first so that `best` picks the last match and
/// `worst` picks the first, exactly like the Python sorter. The field table,
/// alias resolution, limit/closest semantics, and sorting-field backfill all
/// mirror the oracle; only the verbose debug printing is omitted.
///
/// Unknown sort fields behave like the oracle's ad-hoc fields: numeric
/// values rank numerically, other strings rank above all numbers.
use std::cmp::Ordering;

/// One field's contribution to a format's sort key, mirroring the tuples
/// returned by `_calculate_field_preference_from_value`.
#[derive(Debug, Clone, PartialEq)]
enum FieldPreference {
    /// Below every real value, e.g. a missing field (`(-10, 0)`).
    Missing,
    /// Below every real value but above missing (`(-1, value, 0)`).
    Deprioritized(f64),
    /// Numeric rank (`(0, value, ...)`).
    Ranked(f64, f64),
    /// String rank (`(1, value, 0)`); sorts above all numbers.
    Named(String),
}

impl Eq for FieldPreference {}

impl Ord for FieldPreference {
    fn cmp(&self, other: &Self) -> Ordering {
        fn rank(value: &FieldPreference) -> u8 {
            match value {
                FieldPreference::Missing => 0,
                FieldPreference::Deprioritized(_) => 1,
                FieldPreference::Ranked(..) => 2,
                FieldPreference::Named(_) => 3,
            }
        }
        rank(self)
            .cmp(&rank(other))
            .then_with(|| match (self, other) {
                (FieldPreference::Deprioritized(a), FieldPreference::Deprioritized(b)) => {
                    a.total_cmp(b)
                }
                (FieldPreference::Ranked(a, b), FieldPreference::Ranked(c, d)) => {
                    a.total_cmp(c).then_with(|| b.total_cmp(d))
                }
                (FieldPreference::Named(a), FieldPreference::Named(b)) => a.cmp(b),
                _ => Ordering::Equal,
            })
    }
}

impl PartialOrd for FieldPreference {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A raw format field value.
#[derive(Debug, Clone, PartialEq)]
enum RawValue {
    Number(f64),
    Text(String),
    Missing,
}

fn raw_format_value(format: &serde_json::Value, field: &str) -> RawValue {
    match format.get(field) {
        None | Some(serde_json::Value::Null) => RawValue::Missing,
        Some(serde_json::Value::Number(value)) => value
            .as_f64()
            .map(RawValue::Number)
            .unwrap_or(RawValue::Missing),
        Some(serde_json::Value::Bool(value)) => RawValue::Number(if *value { 1.0 } else { 0.0 }),
        Some(serde_json::Value::String(value)) => RawValue::Text(value.clone()),
        Some(_) => RawValue::Missing,
    }
}

/// Mirrors `float_or_none`: numbers convert, numeric strings convert,
/// anything else does not.
fn raw_to_number(value: &RawValue) -> Option<f64> {
    match value {
        RawValue::Number(value) => Some(*value),
        RawValue::Text(value) => value.parse::<f64>().ok(),
        RawValue::Missing => None,
    }
}

/// Whether a raw value counts as present for the `multiple` combinators,
/// mirroring `filter(None, ...)` (drops missing, zero, and empty values).
fn raw_is_present(value: &RawValue) -> bool {
    match value {
        RawValue::Missing => false,
        RawValue::Number(value) => *value != 0.0,
        RawValue::Text(value) => !value.is_empty(),
    }
}

/// Field kinds from `FormatSorter.settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Extractor,
    Boolean,
    Ordered,
    Multiple,
    Plain,
}

/// Combinators for `multiple` fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MultipleFn {
    /// `aud_or_vid`: whether any backing value differs from `'none'`.
    AnyPresent,
    /// `br`, `size`: the first present backing value.
    FirstPresent,
    /// `res`: the smallest present backing value, else `0`.
    MinPresent,
}

#[derive(Debug, Clone, Copy)]
struct FieldSetting {
    kind: FieldKind,
    /// Backing format field; `None` means the sort field itself.
    field: Option<&'static str>,
    fields: Option<&'static [&'static str]>,
    multiple_fn: Option<MultipleFn>,
    not_in_list: Option<&'static [&'static str]>,
    order: Option<&'static [&'static str]>,
    use_regex: bool,
    default: Option<f64>,
    max: Option<f64>,
    forced: bool,
    priority: bool,
}

fn plain(field: Option<&'static str>, default: Option<f64>) -> FieldSetting {
    FieldSetting {
        kind: FieldKind::Plain,
        field,
        fields: None,
        multiple_fn: None,
        not_in_list: None,
        order: None,
        use_regex: false,
        default,
        max: None,
        forced: false,
        priority: false,
    }
}

fn ordered(
    field: Option<&'static str>,
    order: &'static [&'static str],
    use_regex: bool,
) -> FieldSetting {
    FieldSetting {
        kind: FieldKind::Ordered,
        field,
        fields: None,
        multiple_fn: None,
        not_in_list: None,
        order: Some(order),
        use_regex,
        default: None,
        max: None,
        forced: false,
        priority: false,
    }
}

/// The `FormatSorter.settings` table. Ordered tables use the exact Python
/// patterns (matched as prefixes, like `re.match`).
fn field_setting(field: &str) -> Option<FieldSetting> {
    let setting = match field {
        "vcodec" => ordered(
            None,
            &[
                "av0?1",
                r"vp0?9\.0?2",
                "vp0?9",
                "[hx]265|he?vc?",
                "[hx]264|avc",
                "vp0?8",
                "mp4v|h263",
                "theora",
                "",
            ],
            true,
        ),
        "acodec" => ordered(
            None,
            &[
                "[af]lac",
                "wav|aiff",
                "opus",
                "vorbis|ogg",
                "aac",
                "mp?4a?",
                "mp3",
                "ac-?4",
                "e-?a?c-?3",
                "ac-?3",
                "dts",
                "",
            ],
            true,
        ),
        "hdr" => ordered(
            Some("dynamic_range"),
            &[
                "dv",
                "(hdr)?12",
                r"(hdr)?10\+",
                "(hdr)?10",
                "hlg",
                "",
                "sdr",
            ],
            true,
        ),
        "proto" => ordered(
            Some("protocol"),
            &[
                "(ht|f)tps",
                "(ht|f)tp$",
                "m3u8.*",
                ".*dash",
                "websocket_frag",
                "rtmpe?",
                "",
                "ws|websocket",
                "f4",
            ],
            true,
        ),
        "vext" => ordered(
            Some("video_ext"),
            &["mp4", "mov", "webm", "flv", "", "none"],
            false,
        ),
        "aext" => ordered(
            Some("audio_ext"),
            &["m4a", "aac", "mp3", "ogg", "opus", "web[am]", "", "none"],
            true,
        ),
        "hidden" => FieldSetting {
            kind: FieldKind::Extractor,
            field: None,
            fields: None,
            multiple_fn: None,
            not_in_list: None,
            order: None,
            use_regex: false,
            default: None,
            max: Some(-1000.0),
            forced: true,
            priority: false,
        },
        "aud_or_vid" => FieldSetting {
            kind: FieldKind::Multiple,
            field: None,
            fields: Some(&["vcodec", "acodec"]),
            multiple_fn: Some(MultipleFn::AnyPresent),
            not_in_list: None,
            order: None,
            use_regex: false,
            default: None,
            max: None,
            forced: true,
            priority: false,
        },
        "ie_pref" => FieldSetting {
            kind: FieldKind::Extractor,
            field: None,
            fields: None,
            multiple_fn: None,
            not_in_list: None,
            order: None,
            use_regex: false,
            default: None,
            max: None,
            forced: false,
            priority: true,
        },
        "hasvid" => FieldSetting {
            kind: FieldKind::Boolean,
            field: Some("vcodec"),
            fields: None,
            multiple_fn: None,
            not_in_list: Some(&["none"]),
            order: None,
            use_regex: false,
            default: None,
            max: None,
            forced: false,
            priority: true,
        },
        "hasaud" => FieldSetting {
            kind: FieldKind::Boolean,
            field: Some("acodec"),
            fields: None,
            multiple_fn: None,
            not_in_list: Some(&["none"]),
            order: None,
            use_regex: false,
            default: None,
            max: None,
            forced: false,
            priority: false,
        },
        "lang" => plain(Some("language_preference"), Some(-1.0)),
        "quality" => plain(None, Some(-1.0)),
        "filesize" => plain(None, None),
        "fs_approx" => plain(Some("filesize_approx"), None),
        "height" => plain(None, None),
        "width" => plain(None, None),
        "fps" => plain(None, None),
        "channels" => plain(Some("audio_channels"), None),
        "tbr" => plain(None, None),
        "vbr" => plain(None, None),
        "abr" => plain(None, None),
        "asr" => plain(None, None),
        "id" => plain(Some("format_id"), None),
        "source" => plain(Some("source_preference"), Some(-1.0)),
        "br" => FieldSetting {
            kind: FieldKind::Multiple,
            field: None,
            fields: Some(&["tbr", "vbr", "abr"]),
            multiple_fn: Some(MultipleFn::FirstPresent),
            not_in_list: None,
            order: None,
            use_regex: false,
            default: None,
            max: None,
            forced: false,
            priority: false,
        },
        "size" => FieldSetting {
            kind: FieldKind::Multiple,
            field: None,
            fields: Some(&["filesize", "filesize_approx"]),
            multiple_fn: Some(MultipleFn::FirstPresent),
            not_in_list: None,
            order: None,
            use_regex: false,
            default: None,
            max: None,
            forced: false,
            priority: false,
        },
        "res" => FieldSetting {
            kind: FieldKind::Multiple,
            field: None,
            fields: Some(&["height", "width"]),
            multiple_fn: Some(MultipleFn::MinPresent),
            not_in_list: None,
            order: None,
            use_regex: false,
            default: None,
            max: None,
            forced: false,
            priority: false,
        },
        _ => return None,
    };
    Some(setting)
}

/// Alias and deprecated-alias resolution from `FormatSorter.settings`.
/// `combined` fields expand to their sub-fields.
fn resolve_sort_field(field: &str) -> Vec<&'static str> {
    match field {
        "format_id" => vec!["id"],
        "preference" | "extractor" | "extractor_preference" => vec!["ie_pref"],
        "language_preference" => vec!["lang"],
        "source_preference" => vec!["source"],
        "protocol" => vec!["proto"],
        "filesize_approx" => vec!["fs_approx"],
        "audio_channels" => vec!["channels"],
        "dimension" | "resolution" => vec!["res"],
        "extension" => vec!["ext"],
        "bitrate" | "total_bitrate" => vec!["br"],
        "video_bitrate" => vec!["vbr"],
        "audio_bitrate" => vec!["abr"],
        "framerate" => vec!["fps"],
        "filesize_estimate" => vec!["size"],
        "samplerate" => vec!["asr"],
        "video_ext" => vec!["vext"],
        "audio_ext" => vec!["aext"],
        "video_codec" => vec!["vcodec"],
        "audio_codec" => vec!["acodec"],
        "video" | "has_video" => vec!["hasvid"],
        "audio" | "has_audio" => vec!["hasaud"],
        "codec" => vec!["vcodec", "acodec"],
        "ext" => vec!["vext", "aext"],
        _ => vec![Box::leak(field.to_owned().into_boxed_str())],
    }
}

/// The default `FormatSorter.default` order.
const DEFAULT_SORT_ORDER: &[&str] = &[
    "hidden",
    "aud_or_vid",
    "hasvid",
    "ie_pref",
    "lang",
    "quality",
    "res",
    "fps",
    "hdr:12",
    "vcodec",
    "channels",
    "acodec",
    "size",
    "br",
    "asr",
    "proto",
    "ext",
    "hasaud",
    "source",
    "id",
];

/// A resolved limit value, mirroring `_resolve_field_value` results.
#[derive(Debug, Clone, PartialEq)]
enum LimitValue {
    Number(f64),
    Text(String),
}

/// Mirrors `_resolve_field_value` for limit texts and ordered values: order
/// tables resolve to ranks, numeric strings to numbers, anything else stays
/// a string.
fn resolve_sort_value(field: &str, text: &str) -> LimitValue {
    if let Some(setting) = field_setting(field) {
        if setting.kind == FieldKind::Ordered {
            let order = setting.order.unwrap_or(&[]);
            let list_length = order.len() as f64;
            let empty_pos = order
                .iter()
                .position(|entry| entry.is_empty())
                .map(|index| index as f64)
                .unwrap_or(list_length + 1.0);
            let lowered = text.to_ascii_lowercase();
            if setting.use_regex {
                for (index, pattern) in order.iter().enumerate() {
                    if pattern.is_empty() {
                        continue;
                    }
                    let anchored = format!("^(?:{pattern})");
                    if regex::Regex::new(&anchored)
                        .ok()
                        .is_some_and(|matcher| matcher.is_match(&lowered))
                    {
                        return LimitValue::Number(list_length - index as f64);
                    }
                }
                return LimitValue::Number(list_length - empty_pos);
            }
            let position = order
                .iter()
                .position(|entry| *entry == lowered)
                .map(|index| index as f64)
                .unwrap_or(empty_pos);
            return LimitValue::Number(list_length - position);
        }
    }
    match text.parse::<f64>() {
        Ok(value) => LimitValue::Number(value),
        Err(_) => LimitValue::Text(text.to_owned()),
    }
}

struct SortField {
    name: String,
    reverse: bool,
    closest: bool,
    limit: Option<LimitValue>,
}

/// Parse one `-S` item (`[+]<field>[~|:<limit>]`), mirroring the
/// `FormatSorter.regex` handling in `evaluate_params`.
fn parse_sort_item(item: &str) -> Option<(String, bool, bool, Option<String>)> {
    let item = item.trim();
    let matcher = regex::Regex::new(r"^ *(\+)?([a-zA-Z0-9_]+)(([~:])(.*?))? *$").ok()?;
    let captures = matcher.captures(item)?;
    let field = captures.get(2)?.as_str().to_owned();
    let reverse = captures.get(1).is_some();
    let separator = captures.get(4).map(|capture| capture.as_str());
    let limit = captures.get(5).map(|capture| capture.as_str().to_owned());
    Some((field, reverse, separator == Some("~"), limit))
}

/// Build the effective field order: forced fields, priority fields, user
/// fields, extractor fields, then the default order. Mirrors
/// `evaluate_params` without a `format_sort_force` option.
fn build_sort_order(user_fields: &[String], extractor_fields: &[String]) -> Vec<SortField> {
    let mut order = Vec::new();
    // Push one parsed item, expanding combined fields (`ext`, `codec`) and
    // splitting colon-separated limits across sub-fields, like the oracle.
    let mut push_item = |field: String, reverse: bool, closest: bool, limit: Option<String>| {
        let resolved = resolve_sort_field(&field.to_ascii_lowercase());
        let limits = limit
            .as_deref()
            .map(|limit| limit.split(':').map(str::to_owned).collect::<Vec<_>>())
            .unwrap_or_default();
        for (index, sub) in resolved.iter().enumerate() {
            if order.iter().any(|field: &SortField| field.name == *sub) {
                continue;
            }
            // A single field keeps its whole limit text; combined fields
            // split colon-separated limits across sub-fields.
            let sub_limit = if resolved.len() == 1 {
                limit.clone()
            } else {
                limits.get(index).cloned().or_else(|| {
                    // A single limit applies to every sub-field.
                    (limits.len() == 1).then(|| limits[0].clone())
                })
            };
            let (closest, limit) = match sub_limit.as_deref() {
                None => (false, None),
                Some(text) => (closest, Some(resolve_sort_value(sub, text))),
            };
            order.push(SortField {
                name: (*sub).to_owned(),
                reverse,
                closest,
                limit,
            });
        }
    };

    // The default order spells `hdr:12`, so every default entry is parsed
    // like a user item.
    let mut defaults = Vec::new();
    for name in DEFAULT_SORT_ORDER {
        let Some(parsed) = parse_sort_item(name) else {
            continue;
        };
        defaults.push(parsed);
    }
    for (field, reverse, closest, limit) in defaults.iter().cloned().filter(|(field, _, _, _)| {
        field_setting(&field.to_ascii_lowercase()).is_some_and(|setting| setting.forced)
    }) {
        push_item(field, reverse, closest, limit);
    }
    for (field, reverse, closest, limit) in defaults.iter().cloned().filter(|(field, _, _, _)| {
        field_setting(&field.to_ascii_lowercase()).is_some_and(|setting| setting.priority)
    }) {
        push_item(field, reverse, closest, limit);
    }
    for item in user_fields.iter().chain(extractor_fields.iter()) {
        let Some((field, reverse, closest, limit)) = parse_sort_item(item) else {
            continue;
        };
        push_item(field, reverse, closest, limit);
    }
    for (field, reverse, closest, limit) in defaults {
        push_item(field, reverse, closest, limit);
    }
    order
}

/// Calculate one field's preference, mirroring
/// `_calculate_field_preference` plus
/// `_calculate_field_preference_from_value`.
fn field_preference(format: &serde_json::Value, field: &SortField) -> FieldPreference {
    let setting = field_setting(&field.name);
    let backing = |name: &str| raw_format_value(format, name);
    // Step one: resolve the raw value through extractor, boolean, ordered,
    // and multiple handling.
    let raw = match setting.as_ref().map(|setting| setting.kind) {
        Some(FieldKind::Extractor) => {
            let value = raw_to_number(&backing(&field.name));
            let maximum = setting.and_then(|setting| setting.max);
            match value {
                None => RawValue::Number(-1.0),
                Some(value) if maximum.is_some_and(|maximum| value >= maximum) => {
                    RawValue::Number(-1.0)
                }
                Some(value) => RawValue::Number(value),
            }
        }
        Some(FieldKind::Boolean) => {
            let backing_field = setting
                .and_then(|setting| setting.field)
                .unwrap_or(&field.name);
            let value = backing(backing_field);
            // A missing value is neither listed nor excluded, like the
            // oracle (`None in ...` is always false).
            let excluded = matches!(&value, RawValue::Text(text)
                if setting.and_then(|setting| setting.not_in_list).is_some_and(|list| list.contains(&text.as_str())));
            RawValue::Number(if excluded { -1.0 } else { 0.0 })
        }
        Some(FieldKind::Ordered) => {
            let backing_field = setting
                .and_then(|setting| setting.field)
                .unwrap_or(&field.name);
            let text = match backing(backing_field) {
                RawValue::Text(text) => text.to_ascii_lowercase(),
                // A missing value ranks like an unknown one, like the oracle.
                RawValue::Missing => String::new(),
                RawValue::Number(value) => value.to_string(),
            };
            match resolve_sort_value(&field.name, &text) {
                LimitValue::Number(rank) => RawValue::Number(rank),
                LimitValue::Text(_) => RawValue::Missing,
            }
        }
        Some(FieldKind::Multiple) => {
            let fields = setting.and_then(|setting| setting.fields).unwrap_or(&[]);
            let values = fields.iter().map(|name| backing(name)).collect::<Vec<_>>();
            match setting.and_then(|setting| setting.multiple_fn) {
                Some(MultipleFn::AnyPresent) => RawValue::Number(
                    if values
                        .iter()
                        .any(|value| !matches!(value, RawValue::Text(text) if text == "none"))
                    {
                        1.0
                    } else {
                        0.0
                    },
                ),
                Some(MultipleFn::FirstPresent) => values
                    .into_iter()
                    .find(|value| raw_is_present(value))
                    .unwrap_or(RawValue::Missing),
                Some(MultipleFn::MinPresent) => values
                    .into_iter()
                    .filter(|value| raw_is_present(value))
                    .filter_map(|value| raw_to_number(&value))
                    .fold(None, |best: Option<f64>, value| {
                        Some(best.map_or(value, |best| best.min(value)))
                    })
                    .map(RawValue::Number)
                    .unwrap_or(RawValue::Number(0.0)),
                None => RawValue::Missing,
            }
        }
        // Plain fields and ad-hoc unknown fields flow through unchanged.
        _ => backing(&field.name),
    };
    // Step two: coerce to a number with the field default, like
    // `float_or_none(value, default=...)`. Unparseable strings on fields
    // without a default rank above all numbers.
    let default = setting.and_then(|setting| setting.default);
    let number = raw_to_number(&raw).or(default);
    match number {
        Some(value) => numeric_preference(value, field),
        None => match raw {
            RawValue::Text(text) => FieldPreference::Named(text),
            _ => FieldPreference::Missing,
        },
    }
}

/// Apply the reverse/limit/closest branches for a numeric value.
fn numeric_preference(value: f64, field: &SortField) -> FieldPreference {
    let limit = match field.limit.as_ref() {
        // A text limit cannot bound a number; the oracle would fail here,
        // while native selection treats the limit as absent.
        Some(LimitValue::Text(_)) | None => None,
        Some(LimitValue::Number(limit)) => Some(*limit),
    };
    if field.closest {
        // `(0, -abs(value - limit), value - limit if reverse else
        // limit - value)`; closest is only set when a limit is present.
        let bound = limit.unwrap_or(0.0);
        let distance = (value - bound).abs();
        let signed = if field.reverse {
            value - bound
        } else {
            bound - value
        };
        return FieldPreference::Ranked(-distance, signed);
    }
    if !field.reverse {
        if limit.is_none_or(|limit| value <= limit) {
            FieldPreference::Ranked(value, 0.0)
        } else {
            FieldPreference::Deprioritized(value)
        }
    } else if limit.is_none() || limit.is_some_and(|limit| value == limit || value > limit) {
        FieldPreference::Ranked(-value, 0.0)
    } else {
        FieldPreference::Deprioritized(value)
    }
}

fn format_number(format: &serde_json::Value, field: &str) -> Option<f64> {
    raw_to_number(&raw_format_value(format, field))
}

/// Backfill the derived sorting fields, mirroring
/// `FormatSorter._fill_sorting_fields`.
fn fill_sorting_fields(format: &mut serde_json::Value) {
    let protocol_missing = format.get("protocol").is_none();
    if protocol_missing {
        let mut info = InfoDict::new();
        if let Some(url) = format.get("url").and_then(serde_json::Value::as_str) {
            info.insert("url", serde_json::json!(url));
        }
        if let Some(protocol) = format.get("protocol").and_then(serde_json::Value::as_str) {
            info.insert("protocol", serde_json::json!(protocol));
        }
        if let Some(is_live) = format.get("is_live").and_then(serde_json::Value::as_bool) {
            info.insert("is_live", serde_json::json!(is_live));
        }
        if let Ok(protocol) = yt_dlp_core::determine_protocol(&info) {
            if let Some(object) = format.as_object_mut() {
                object.insert("protocol".to_owned(), serde_json::json!(protocol));
            }
        }
    }
    let ext_missing = format.get("ext").is_none();
    if ext_missing {
        if let Some(url) = format.get("url").and_then(serde_json::Value::as_str) {
            let ext = yt_dlp_core::determine_ext(Some(url), "").to_ascii_lowercase();
            if !ext.is_empty() {
                if let Some(object) = format.as_object_mut() {
                    object.insert("ext".to_owned(), serde_json::json!(ext));
                }
            }
        }
    }
    let vcodec_none = format.get("vcodec").and_then(serde_json::Value::as_str) == Some("none");
    if let Some(object) = format.as_object_mut() {
        if vcodec_none {
            let acodec_none =
                object.get("acodec").and_then(serde_json::Value::as_str) == Some("none");
            object.insert(
                "audio_ext".to_owned(),
                if acodec_none {
                    serde_json::json!("none")
                } else {
                    object
                        .get("ext")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null)
                },
            );
            object.insert("video_ext".to_owned(), serde_json::json!("none"));
        } else {
            object.insert(
                "video_ext".to_owned(),
                object
                    .get("ext")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            );
            object.insert("audio_ext".to_owned(), serde_json::json!("none"));
        }
    }
    // HEVC-over-FLV is out of spec; deprioritize it like the oracle.
    let flv_hevc = format.get("ext").and_then(serde_json::Value::as_str) == Some("flv")
        && format
            .get("vcodec")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|vcodec| {
                regex::Regex::new("^(?:[hx]265|he?vc?)")
                    .ok()
                    .is_some_and(|matcher| matcher.is_match(vcodec))
            });
    if flv_hevc && format.get("preference").is_none() {
        if let Some(object) = format.as_object_mut() {
            object.insert("preference".to_owned(), serde_json::json!(-100));
        }
    }
    if vcodec_none {
        if let Some(object) = format.as_object_mut() {
            object.insert("vbr".to_owned(), serde_json::json!(0));
        }
    }
    let acodec_none = format.get("acodec").and_then(serde_json::Value::as_str) == Some("none");
    if acodec_none {
        if let Some(object) = format.as_object_mut() {
            object.insert("abr".to_owned(), serde_json::json!(0));
        }
    }
    let vbr = format_number(format, "vbr");
    let abr = format_number(format, "abr");
    let tbr = format_number(format, "tbr");
    if vbr.unwrap_or(0.0) == 0.0
        && format.get("vcodec").and_then(serde_json::Value::as_str) != Some("none")
    {
        if let (Some(tbr), Some(abr)) = (tbr, abr) {
            if let Some(object) = format.as_object_mut() {
                object.insert("vbr".to_owned(), serde_json::json!(tbr - abr));
            }
        }
    }
    if abr.unwrap_or(0.0) == 0.0
        && format.get("acodec").and_then(serde_json::Value::as_str) != Some("none")
    {
        let vbr = format_number(format, "vbr");
        if let (Some(tbr), Some(vbr)) = (tbr, vbr) {
            if let Some(object) = format.as_object_mut() {
                object.insert("abr".to_owned(), serde_json::json!(tbr - vbr));
            }
        }
    }
    // `not format.get('tbr')` also triggers on a zero bitrate.
    if format_number(format, "tbr").unwrap_or(0.0) == 0.0 {
        let vbr = format_number(format, "vbr");
        let abr = format_number(format, "abr");
        if let (Some(vbr), Some(abr)) = (vbr, abr) {
            if let Some(object) = format.as_object_mut() {
                object.insert("tbr".to_owned(), serde_json::json!(vbr + abr));
            }
        }
    }
}

fn format_preference_key(format: &serde_json::Value, order: &[SortField]) -> Vec<FieldPreference> {
    order
        .iter()
        .map(|field| field_preference(format, field))
        .collect()
}

/// Sort formats worst-first, mirroring `YoutubeDL.sort_formats`.
/// `user_fields` are the `-S` sort strings; `extractor_fields` are the
/// extractor-provided `_format_sort_fields`.
fn sort_native_formats(
    formats: &mut [serde_json::Value],
    user_fields: &[String],
    extractor_fields: &[String],
) {
    for format in formats.iter_mut() {
        fill_sorting_fields(format);
    }
    let order = build_sort_order(user_fields, extractor_fields);
    formats.sort_by(|left, right| {
        format_preference_key(left, &order).cmp(&format_preference_key(right, &order))
    });
}
