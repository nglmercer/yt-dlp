/// Native format selection: selector parsing, filtering, merging, and
/// multi-format orchestration support (`yt_dlp/YoutubeDL.py`).
///
/// Supported selector syntax mirrors the oracle: `/` fallbacks, `+` merges,
/// `,` concatenations, `(...)` groups, `best`/`worst` atoms with
/// audio/video/`*`/`.N` modifiers, extension atoms, format IDs, `all`, and
/// `mergeall`, plus `[...]` filters. Formats are sorted worst-first with the
/// native `FormatSorter` port before selection.

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedFormat {
    url: String,
    ext: Option<String>,
    extra_param_to_segment_url: Option<String>,
}

/// One downloadable unit: a single format, or a merged virtual format with
/// its component `requested_formats`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NativeSelection {
    Single(serde_json::Value),
    Merged(serde_json::Value),
}

#[cfg(test)]
fn select_download_format(
    info: &InfoDict,
    selector: Option<&str>,
) -> Result<(String, Option<String>), String> {
    let selections = select_native_downloads(info, selector, &cli::CliOptions::default())?;
    let Some(selection) = selections.into_iter().next() else {
        return Err("no native format matches selector".to_owned());
    };
    let NativeSelection::Single(format) = selection else {
        return Err("selector unexpectedly merged formats".to_owned());
    };
    let selected = selected_format_details(&format, info)?;
    Ok((selected.url, selected.ext))
}

fn selected_format_details(
    format: &serde_json::Value,
    info: &InfoDict,
) -> Result<SelectedFormat, String> {
    let url = format
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "selected native format has no URL".to_owned())?;
    let ext = format
        .get("ext")
        .and_then(serde_json::Value::as_str)
        .or_else(|| info.get("ext").and_then(serde_json::Value::as_str))
        .map(str::to_owned);
    Ok(SelectedFormat {
        url: url.to_owned(),
        ext,
        extra_param_to_segment_url: format_extra_param(format)?,
    })
}

fn format_extra_param(format: &serde_json::Value) -> Result<Option<String>, String> {
    let Some(value) = format.get("extra_param_to_segment_url") else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|value| Some(value.to_owned()))
        .ok_or_else(|| "TODO: native format extra_param_to_segment_url must be a string".to_owned())
}

/// Extension atoms from `YoutubeDL._format_selection_exts`. The video set is
/// exactly `{*common_video, '3gp'}` like the oracle.
fn audio_selection_exts() -> &'static [&'static str] {
    &[
        "aac", "ape", "asf", "f4a", "f4b", "m4b", "m4r", "oga", "ogx", "spx", "vorbis", "wma",
        "weba", "aiff", "alac", "flac", "m4a", "mka", "mp3", "ogg", "opus", "wav",
    ]
}

fn video_selection_exts() -> &'static [&'static str] {
    &["avi", "flv", "mkv", "mov", "mp4", "webm", "3gp"]
}

fn storyboard_selection_exts() -> &'static [&'static str] {
    &["mhtml"]
}

/// First-present traversal over several keys, like `traverse_obj(fmt, *keys)`.
fn first_present<'a>(
    format: &'a serde_json::Value,
    keys: &[&str],
) -> Option<&'a serde_json::Value> {
    keys.iter()
        .filter_map(|key| format.get(*key))
        .find(|value| !value.is_null() && value.as_str().map_or(true, |text| !text.is_empty()))
}

/// Ordered deduplication, mirroring `orderedSet`.
fn ordered_set(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

/// Sanitize one codec for container compatibility, mirroring the
/// `sanitize_codec` partial in `get_compatible_ext`: the first codec token
/// before `.`, with `0` removed, lowercased.
fn sanitize_compatible_codec(codec: Option<&str>) -> Option<String> {
    codec.map(|codec| {
        codec
            .split('.')
            .next()
            .unwrap_or(codec)
            .replace('0', "")
            .to_ascii_lowercase()
    })
}

/// Port of `get_compatible_ext`: pick the output extension for merged
/// audio/video codecs and extensions.
fn compatible_merge_ext(
    vcodecs: &[Option<String>],
    acodecs: &[Option<String>],
    vexts: &[String],
    aexts: &[String],
    preferences: Option<&[String]>,
) -> String {
    let allow_mkv = preferences.is_none_or(|preferences| preferences.contains(&"mkv".to_owned()));
    if allow_mkv && vcodecs.len().max(acodecs.len()) > 1 {
        return "mkv".to_owned();
    }
    let vcodec = vcodecs
        .first()
        .and_then(|codec| sanitize_compatible_codec(codec.as_deref()));
    let acodec = acodecs
        .first()
        .and_then(|codec| sanitize_compatible_codec(codec.as_deref()));
    let preference_list;
    let preference_iter: &[String] = match preferences {
        Some(preferences) => {
            preference_list = preferences.to_vec();
            &preference_list
        }
        None => {
            preference_list = vec!["mp4".to_owned(), "webm".to_owned()];
            &preference_list
        }
    };
    for ext in preference_iter {
        let codecs: std::collections::BTreeSet<Option<String>> = match ext.as_str() {
            "mp4" => [
                "av1", "hevc", "avc1", "mp4a", "ac-4", "h264", "aacl", "ec-3",
            ]
            .into_iter()
            .map(|codec| Some(codec.to_owned()))
            .collect(),
            "webm" => ["av1", "vp9", "vp8", "opus", "vrbs", "vp9x", "vp8x"]
                .into_iter()
                .map(|codec| Some(codec.to_owned()))
                .collect(),
            _ => std::collections::BTreeSet::new(),
        };
        let wanted: std::collections::BTreeSet<Option<String>> =
            [vcodec.clone(), acodec.clone()].into_iter().collect();
        if ext == "mkv" || codecs.is_superset(&wanted) {
            return ext.clone();
        }
    }
    let compatible_sets: &[std::collections::BTreeSet<&str>] = &[
        [
            "mp3", "mp4", "m4a", "m4p", "m4b", "m4r", "m4v", "ismv", "isma", "mov",
        ]
        .into_iter()
        .collect(),
        ["webm", "weba"].into_iter().collect(),
    ];
    let candidates: Vec<&String> = match preferences {
        Some(preferences) => preferences.iter().collect(),
        None => vexts.iter().collect(),
    };
    for ext in candidates {
        let mut current = std::collections::BTreeSet::new();
        current.insert(ext.as_str());
        current.extend(vexts.iter().map(String::as_str));
        current.extend(aexts.iter().map(String::as_str));
        if ext == "mkv"
            || current.len() == 1
            || compatible_sets.iter().any(|set| set.is_superset(&current))
        {
            return (*ext).clone();
        }
    }
    if allow_mkv {
        "mkv".to_owned()
    } else {
        preferences
            .and_then(|preferences| preferences.last().cloned())
            .unwrap_or_else(|| "mkv".to_owned())
    }
}

#[derive(Debug, Clone, PartialEq)]
enum FilterOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, PartialEq)]
enum FilterValue {
    Number(f64),
    Text(String),
    Pattern(String),
}

struct FormatFilter {
    key: String,
    negate: bool,
    none_inclusive: bool,
    numeric_op: Option<FilterOp>,
    string_op: Option<StringOp>,
    value: FilterValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StringOp {
    Eq,
    Prefix,
    Suffix,
    Contains,
    Regex,
}

/// Parse a filesize quantity with the short suffixes accepted by format
/// filters, mirroring `parse_filesize` (`KB` is decimal, `KiB` is binary).
fn parse_filter_filesize(text: &str) -> Option<f64> {
    let text = text.trim();
    let split = text
        .char_indices()
        .find(|(_, character)| !(character.is_ascii_digit() || *character == '.'))
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    let (number, suffix) = text.split_at(split);
    let number = number.parse::<f64>().ok()?;
    if suffix.is_empty() {
        return Some(number);
    }
    let exponent = match suffix.chars().next()? {
        'K' | 'k' => 1,
        'M' | 'm' => 2,
        'G' | 'g' => 3,
        'T' | 't' => 4,
        'P' | 'p' => 5,
        'E' | 'e' => 6,
        'Z' | 'z' => 7,
        'Y' | 'y' => 8,
        _ => return None,
    };
    let rest = &suffix[1..];
    let base: f64 = if rest.eq_ignore_ascii_case("ib") {
        1024.0
    } else if rest.eq_ignore_ascii_case("b") || rest.is_empty() {
        // `kB` is binary, `KB`/`Kb`/`kb` are decimal, like the oracle.
        if rest == "B"
            && suffix
                .chars()
                .next()
                .is_some_and(|letter| letter.is_ascii_lowercase())
        {
            1024.0
        } else if suffix.len() == 1 {
            // Handled by the caller appending `B`; unreachable here.
            return None;
        } else {
            1000.0
        }
    } else {
        return None;
    };
    Some(number * base.powi(exponent))
}

/// Build one `[...]` format filter, mirroring `_build_format_filter`.
fn build_format_filter(filter_spec: &str) -> Result<FormatFilter, String> {
    let invalid = || format!("Invalid filter specification {filter_spec:?}");
    // Numeric form: `<key><op><value>[?]`.
    let numeric = regex::Regex::new(
        r"^\s*([\w.\-]+)\s*(<=|>=|!=|=|<|>)\s*(\?\s*)?([0-9.]+(?:[kKmMgGtTpPeEzZyY]i?[Bb]?)?)\s*$",
    )
    .map_err(|_| invalid())?;
    if let Some(captures) = numeric.captures(filter_spec) {
        let value_text = captures
            .get(4)
            .map(|capture| capture.as_str())
            .unwrap_or("");
        let value = value_text.parse::<f64>().ok().or_else(|| {
            parse_filter_filesize(value_text)
                .or_else(|| parse_filter_filesize(&format!("{value_text}B")))
        });
        let Some(value) = value else {
            return Err(format!(
                "Invalid value {value_text:?} in format specification {filter_spec:?}"
            ));
        };
        let op = match captures.get(2).map(|capture| capture.as_str()) {
            Some("<") => FilterOp::Lt,
            Some("<=") => FilterOp::Le,
            Some(">") => FilterOp::Gt,
            Some(">=") => FilterOp::Ge,
            Some("=") => FilterOp::Eq,
            Some("!=") => FilterOp::Ne,
            _ => return Err(invalid()),
        };
        return Ok(FormatFilter {
            key: captures
                .get(1)
                .map(|capture| capture.as_str())
                .unwrap_or("")
                .to_owned(),
            negate: false,
            none_inclusive: captures.get(3).is_some(),
            numeric_op: Some(op),
            string_op: None,
            value: FilterValue::Number(value),
        });
    }
    // String form: `<key>[!]<op>[?]<value>`.
    let (key, rest) = parse_filter_key(filter_spec)?;
    let rest = rest.trim_start();
    let (negate, rest) = match rest.strip_prefix('!') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, rest),
    };
    let (op, rest) = ["^=", "$=", "*=", "~=", "="]
        .into_iter()
        .find_map(|op| rest.strip_prefix(op).map(|rest| (op, rest)))
        .ok_or_else(invalid)?;
    let rest = rest.trim_start();
    let (none_inclusive, rest) = match rest.strip_prefix('?') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, rest),
    };
    let value = parse_filter_string_value(rest)?;
    let (string_op, value) = match op {
        "=" => (StringOp::Eq, FilterValue::Text(value)),
        "^=" => (StringOp::Prefix, FilterValue::Text(value)),
        "$=" => (StringOp::Suffix, FilterValue::Text(value)),
        "*=" => (StringOp::Contains, FilterValue::Text(value)),
        "~=" => {
            regex::Regex::new(&value).map_err(|_| invalid())?;
            (StringOp::Regex, FilterValue::Pattern(value))
        }
        _ => return Err(invalid()),
    };
    Ok(FormatFilter {
        key,
        negate,
        none_inclusive,
        numeric_op: None,
        string_op: Some(string_op),
        value,
    })
}

/// Split a string filter into its key and remainder.
fn parse_filter_key(filter_spec: &str) -> Result<(String, &str), String> {
    let end = filter_spec
        .char_indices()
        .find(|(_, character)| {
            !(character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | ' '))
        })
        .map(|(index, _)| index)
        .unwrap_or(filter_spec.len());
    let key = filter_spec[..end].trim();
    if key.is_empty() {
        return Err(format!("Invalid filter specification {filter_spec:?}"));
    }
    Ok((key.to_owned(), &filter_spec[end..]))
}

/// Parse a string filter value: a quoted string or a bare `[\w.-]+` word,
/// with `\"`/`\'` unescaping like the oracle.
fn parse_filter_string_value(rest: &str) -> Result<String, String> {
    let invalid = || format!("Invalid filter specification near {rest:?}");
    let rest = rest.trim_end();
    let Some(first) = rest.chars().next() else {
        return Err(invalid());
    };
    if first == '"' || first == '\'' {
        let mut value = String::new();
        let mut chars = rest[1..].chars();
        loop {
            let Some(character) = chars.next() else {
                return Err(invalid());
            };
            if character == '\\' {
                let Some(escaped) = chars.next() else {
                    return Err(invalid());
                };
                if escaped == '"' || escaped == '\'' {
                    value.push(escaped);
                } else {
                    value.push(character);
                    value.push(escaped);
                }
            } else if character == first {
                break;
            } else {
                value.push(character);
            }
        }
        if !chars.as_str().trim().is_empty() {
            return Err(invalid());
        }
        return Ok(value);
    }
    let end = rest
        .char_indices()
        .find(|(_, character)| {
            !(character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '-'))
        })
        .map(|(index, _)| index)
        .unwrap_or(rest.len());
    let (value, trailing) = rest.split_at(end);
    if value.is_empty() || !trailing.trim().is_empty() {
        return Err(invalid());
    }
    Ok(value.to_owned())
}

fn apply_filter_numeric(actual: f64, op: &FilterOp, expected: f64) -> bool {
    match op {
        FilterOp::Lt => actual < expected,
        FilterOp::Le => actual <= expected,
        FilterOp::Gt => actual > expected,
        FilterOp::Ge => actual >= expected,
        FilterOp::Eq => actual == expected,
        FilterOp::Ne => actual != expected,
    }
}

/// Test one format against a filter, mirroring the `_filter` closure.
fn format_matches_filter(format: &serde_json::Value, filter: &FormatFilter) -> bool {
    let actual = format.get(&filter.key);
    if actual.is_none() || actual == Some(&serde_json::Value::Null) {
        return filter.none_inclusive;
    }
    let actual = actual.expect("checked above");
    if let Some(op) = filter.numeric_op.as_ref() {
        let FilterValue::Number(expected) = filter.value else {
            return false;
        };
        let Some(actual) = (match actual {
            serde_json::Value::Number(value) => value.as_f64(),
            serde_json::Value::String(value) => value.parse::<f64>().ok(),
            serde_json::Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
            _ => None,
        }) else {
            // The oracle raises here; native selection treats the format as
            // not matching instead of failing the whole selection.
            return false;
        };
        return apply_filter_numeric(actual, op, expected);
    }
    let Some(op) = filter.string_op else {
        return false;
    };
    let actual = match actual {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        _ => return false,
    };
    let matched = match op {
        StringOp::Eq => match &filter.value {
            FilterValue::Text(expected) => actual == *expected,
            _ => false,
        },
        StringOp::Prefix => match &filter.value {
            FilterValue::Text(expected) => actual.starts_with(expected),
            _ => false,
        },
        StringOp::Suffix => match &filter.value {
            FilterValue::Text(expected) => actual.ends_with(expected),
            _ => false,
        },
        StringOp::Contains => match &filter.value {
            FilterValue::Text(expected) => actual.contains(expected),
            _ => false,
        },
        StringOp::Regex => match &filter.value {
            FilterValue::Pattern(pattern) => regex::Regex::new(pattern)
                .ok()
                .is_some_and(|matcher| matcher.is_match(&actual)),
            _ => false,
        },
    };
    matched != filter.negate
}

/// A parsed format selector, mirroring the `PICKFIRST`/`MERGE`/`SINGLE`/
/// `GROUP` nodes from `build_format_selector`.
#[derive(Debug, Clone)]
enum FormatSelector {
    Single { spec: String, filters: Vec<String> },
    PickFirst(Box<FormatSelector>, Box<FormatSelector>),
    Merge(Box<FormatSelector>, Box<FormatSelector>),
    Group(Vec<FormatSelector>),
    Concat(Vec<FormatSelector>),
}

struct SelectorParser<'a> {
    spec: &'a str,
    chars: Vec<char>,
    pos: usize,
}

impl<'a> SelectorParser<'a> {
    fn syntax_error(&self, note: &str) -> String {
        format!("Invalid format specification: {note}: {}", self.spec)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let character = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        Some(character)
    }

    fn skip_ws(&mut self) {
        while self
            .peek()
            .is_some_and(|character| character.is_whitespace())
        {
            self.pos += 1;
        }
    }

    /// Parse one level, mirroring `_parse_format_selection` with its
    /// `inside_merge`/`inside_choice`/`inside_group` flags.
    fn parse_level(
        &mut self,
        inside_merge: bool,
        inside_choice: bool,
        inside_group: bool,
    ) -> Result<Vec<FormatSelector>, String> {
        let mut selectors = Vec::new();
        let mut current: Option<FormatSelector> = None;
        loop {
            self.skip_ws();
            match self.peek() {
                None => {
                    if inside_group {
                        return Err(
                            self.syntax_error("Missing closing/opening brackets or parenthesis")
                        );
                    }
                    break;
                }
                Some(')') => {
                    if inside_group {
                        self.next();
                    }
                    break;
                }
                Some(',') => {
                    if inside_merge || inside_choice {
                        break;
                    }
                    self.next();
                    let node = current
                        .take()
                        .ok_or_else(|| self.syntax_error("\",\" must follow a format selector"))?;
                    selectors.push(node);
                }
                Some('/') => {
                    if inside_merge {
                        break;
                    }
                    self.next();
                    let first = current
                        .take()
                        .ok_or_else(|| self.syntax_error("\"/\" must follow a format selector"))?;
                    let second = Self::single_or_concat(self.parse_level(false, true, false)?);
                    current = Some(FormatSelector::PickFirst(Box::new(first), Box::new(second)));
                }
                Some('[') => {
                    let mut node = current.take().unwrap_or(FormatSelector::Single {
                        spec: "best".to_owned(),
                        filters: Vec::new(),
                    });
                    let FormatSelector::Single { filters, .. } = &mut node else {
                        return Err(self.syntax_error("filters require a plain format selector"));
                    };
                    filters.push(self.parse_filter()?);
                    current = Some(node);
                }
                Some('(') => {
                    if current.is_some() {
                        return Err(self.syntax_error("Unexpected \"(\""));
                    }
                    self.next();
                    let group = self.parse_level(false, false, true)?;
                    current = Some(FormatSelector::Group(group));
                }
                Some('+') => {
                    self.next();
                    let first = current
                        .take()
                        .ok_or_else(|| self.syntax_error("Unexpected \"+\""))?;
                    let second = self.parse_level(true, false, false)?;
                    if second.is_empty() {
                        return Err(self.syntax_error("Expected a selector"));
                    }
                    current = Some(FormatSelector::Merge(
                        Box::new(first),
                        Box::new(Self::single_or_concat(second)),
                    ));
                }
                Some(_) => {
                    let spec = self.parse_atom()?;
                    current = Some(FormatSelector::Single {
                        spec,
                        filters: Vec::new(),
                    });
                }
            }
        }
        if let Some(node) = current {
            selectors.push(node);
        }
        Ok(selectors)
    }

    fn single_or_concat(selectors: Vec<FormatSelector>) -> FormatSelector {
        if selectors.len() == 1 {
            selectors.into_iter().next().expect("checked above")
        } else {
            FormatSelector::Concat(selectors)
        }
    }

    /// Read one atom spec up to the next operator.
    fn parse_atom(&mut self) -> Result<String, String> {
        let start = self.pos;
        while let Some(character) = self.peek() {
            if matches!(character, ',' | '/' | '+' | '(' | ')' | '[' | ']') {
                break;
            }
            self.pos += 1;
        }
        let spec = self.chars[start..self.pos].iter().collect::<String>();
        let spec = spec.trim().to_owned();
        if spec.is_empty() {
            return Err(self.syntax_error("Expected a format selector"));
        }
        Ok(spec)
    }

    /// Read a `[...]` filter body, keeping quoted `]` characters intact.
    fn parse_filter(&mut self) -> Result<String, String> {
        self.next();
        let mut body = String::new();
        let mut quote = None;
        loop {
            let Some(character) = self.next() else {
                return Err(self.syntax_error("Missing closing/opening brackets or parenthesis"));
            };
            if let Some(active) = quote {
                body.push(character);
                if character == '\\' {
                    if let Some(escaped) = self.next() {
                        body.push(escaped);
                    }
                } else if character == active {
                    quote = None;
                }
            } else if character == '"' || character == '\'' {
                quote = Some(character);
                body.push(character);
            } else if character == ']' {
                break;
            } else {
                body.push(character);
            }
        }
        // Join the filter tokens like `_parse_filter`: whitespace outside
        // quotes is insignificant.
        let mut joined = String::new();
        let mut chars = body.chars().peekable();
        let mut active = None;
        while let Some(character) = chars.next() {
            if let Some(quote) = active {
                joined.push(character);
                if character == '\\' {
                    if let Some(escaped) = chars.next() {
                        joined.push(escaped);
                    }
                } else if character == quote {
                    active = None;
                }
            } else if character == '"' || character == '\'' {
                active = Some(character);
                joined.push(character);
            } else if !character.is_whitespace() {
                joined.push(character);
            }
        }
        Ok(joined)
    }
}

fn parse_format_selector(spec: &str) -> Result<Vec<FormatSelector>, String> {
    let mut parser = SelectorParser {
        spec,
        chars: spec.chars().collect(),
        pos: 0,
    };
    parser.parse_level(false, false, false)
}

struct SelectionContext {
    has_merged_format: bool,
    incomplete_formats: bool,
}

fn selection_context(formats: &[serde_json::Value]) -> SelectionContext {
    SelectionContext {
        has_merged_format: formats.iter().any(|format| {
            format.get("vcodec").and_then(serde_json::Value::as_str) != Some("none")
                && format.get("acodec").and_then(serde_json::Value::as_str) != Some("none")
        }),
        incomplete_formats: formats.iter().any(|format| {
            format.get("vcodec").and_then(serde_json::Value::as_str) == Some("none")
                || format.get("acodec").and_then(serde_json::Value::as_str) == Some("none")
        }),
    }
}

fn format_has_video(format: &serde_json::Value) -> bool {
    format.get("vcodec").and_then(serde_json::Value::as_str) != Some("none")
}

fn format_has_audio(format: &serde_json::Value) -> bool {
    format.get("acodec").and_then(serde_json::Value::as_str) != Some("none")
}

/// The 1-based `.N` pick of a best/worst atom, if the spec is one.
fn best_worst_pick_index(spec: &str) -> Option<usize> {
    let captures = regex::Regex::new(r"^(best|b|worst|w)(video|audio|v|a)?(\*)?(?:\.([1-9]\d*))?$")
        .ok()
        .and_then(|matcher| matcher.captures(spec))?;
    Some(
        captures
            .get(4)
            .and_then(|capture| capture.as_str().parse::<usize>().ok())
            .unwrap_or(1),
    )
}

/// Evaluate one `SINGLE` atom, mirroring `_select_formats`'s single-format
/// branch. Formats arrive worst-first; `reverse` picks from the best end.
fn eval_atom(
    spec: &str,
    formats: &[serde_json::Value],
    ctx: &SelectionContext,
    merge_output_format: Option<&str>,
) -> Vec<serde_json::Value> {
    if spec == "mergeall" {
        // Fold from the best format downward, skipping data-only streams.
        let mut usable = formats
            .iter()
            .filter(|format| format_has_video(format) || format_has_audio(format))
            .rev();
        let Some(best) = usable.next() else {
            return Vec::new();
        };
        let mut merged = best.clone();
        for format in usable {
            merged = merge_format_pair(&merged, format, merge_output_format);
        }
        return vec![merged];
    }
    if spec == "all" {
        return formats.iter().rev().cloned().collect();
    }
    if let Some(captures) =
        regex::Regex::new(r"^(best|b|worst|w)(video|audio|v|a)?(\*)?(?:\.([1-9]\d*))?$")
            .ok()
            .and_then(|matcher| matcher.captures(spec))
    {
        let best = matches!(
            captures.get(1).map(|capture| capture.as_str()),
            Some("best") | Some("b")
        );
        let media = captures.get(2).map(|capture| capture.as_str());
        let any_media = captures.get(3).is_some();
        let mut matches: Vec<&serde_json::Value> = formats
            .iter()
            .filter(|format| match media {
                Some("video") | Some("v") => {
                    format_has_video(format) && (any_media || !format_has_audio(format))
                }
                Some("audio") | Some("a") => {
                    format_has_audio(format) && (any_media || !format_has_video(format))
                }
                _ => format_has_video(format) && format_has_audio(format),
            })
            .collect();
        if best {
            matches.reverse();
        }
        // Without merged formats, plain `best`/`worst` fall back to any
        // format carrying at least one stream, like the oracle.
        if matches.is_empty() && media.is_none() && ctx.incomplete_formats {
            matches = formats
                .iter()
                .filter(|format| format_has_video(format) || format_has_audio(format))
                .collect();
            if best {
                matches.reverse();
            }
        }
        // The `.N` truncation happens after filtering, like the oracle,
        // so every match in pick order is returned here.
        return matches.into_iter().cloned().collect();
    }
    // Extension and ID atoms yield their single best match; only `all`
    // downloads every match.
    if audio_selection_exts().contains(&spec) {
        return formats
            .iter()
            .rev()
            .find(|format| {
                format.get("ext").and_then(serde_json::Value::as_str) == Some(spec)
                    && format_has_audio(format)
            })
            .into_iter()
            .cloned()
            .collect();
    }
    if video_selection_exts().contains(&spec) {
        let found = formats
            .iter()
            .rev()
            .find(|format| {
                format.get("ext").and_then(serde_json::Value::as_str) == Some(spec)
                    && format_has_audio(format)
                    && format_has_video(format)
            })
            .cloned();
        if found.is_some() || ctx.has_merged_format {
            return found.into_iter().collect();
        }
        // Without merged formats, a video extension falls back to the first
        // video-only format of that container, in ascending order.
        return formats
            .iter()
            .find(|format| {
                format.get("ext").and_then(serde_json::Value::as_str) == Some(spec)
                    && format_has_video(format)
            })
            .into_iter()
            .cloned()
            .collect();
    }
    if storyboard_selection_exts().contains(&spec) {
        return formats
            .iter()
            .rev()
            .find(|format| {
                format.get("ext").and_then(serde_json::Value::as_str) == Some(spec)
                    && !format_has_audio(format)
                    && !format_has_video(format)
            })
            .into_iter()
            .cloned()
            .collect();
    }
    // Otherwise the atom is a format ID; the oracle also accepts the
    // `format_id`-with-extension and `f`-prefixed spellings.
    formats
        .iter()
        .rev()
        .find(|format| {
            let Some(format_id) = format.get("format_id").and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            format_id == spec
                || Some(format!("{format_id}-{spec}"))
                    == format
                        .get("ext")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                || format!("f{format_id}") == spec
        })
        .into_iter()
        .cloned()
        .collect()
}

fn format_protocol(format: &serde_json::Value) -> String {
    if let Some(protocol) = format.get("protocol").and_then(serde_json::Value::as_str) {
        return protocol.to_owned();
    }
    let mut info = InfoDict::new();
    if let Some(url) = format.get("url").and_then(serde_json::Value::as_str) {
        info.insert("url", serde_json::json!(url));
    }
    yt_dlp_core::determine_protocol(&info)
        .map(|protocol| protocol.to_string())
        .unwrap_or_else(|_| "http".to_owned())
}

/// Merge two selected formats into one virtual format, mirroring
/// `YoutubeDL._merge`. Data-only streams and duplicate audio/video streams
/// are removed unless the caller allows multiples (native selection never
/// does, matching the default options).
fn merge_format_pair(
    first: &serde_json::Value,
    second: &serde_json::Value,
    merge_output_format: Option<&str>,
) -> serde_json::Value {
    let mut parts = Vec::new();
    for format in [first, second] {
        match format
            .get("requested_formats")
            .and_then(serde_json::Value::as_array)
        {
            Some(requested) => parts.extend(requested.iter().cloned()),
            None => parts.push(format.clone()),
        }
    }
    // Remove data-only streams and duplicate audio/video streams, following
    // the oracle's index walk (including its skip-after-pop behaviour).
    let mut audio_found = false;
    let mut video_found = false;
    let mut index = 0;
    while index < parts.len() {
        let is_data_only = !format_has_audio(&parts[index]) && !format_has_video(&parts[index]);
        let mut drop = is_data_only;
        if !drop {
            for stream in ["audio", "video"] {
                let present = if stream == "audio" {
                    format_has_audio(&parts[index])
                } else {
                    format_has_video(&parts[index])
                };
                if !present {
                    continue;
                }
                let already = if stream == "audio" {
                    &mut audio_found
                } else {
                    &mut video_found
                };
                if *already {
                    drop = true;
                    break;
                }
                *already = true;
            }
        }
        if drop {
            parts.remove(index);
        }
        index += 1;
    }
    if parts.len() == 1 {
        return parts.into_iter().next().expect("checked above");
    }
    let vcodecs = parts
        .iter()
        .filter(|format| format_has_video(format))
        .map(|format| {
            format
                .get("vcodec")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let acodecs = parts
        .iter()
        .filter(|format| format_has_audio(format))
        .map(|format| {
            format
                .get("acodec")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let vexts = parts
        .iter()
        .filter(|format| format_has_video(format))
        .filter_map(|format| {
            format
                .get("ext")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let aexts = parts
        .iter()
        .filter(|format| format_has_audio(format))
        .filter_map(|format| {
            format
                .get("ext")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let preferences = merge_output_format
        .map(|formats| formats.split('/').map(str::to_owned).collect::<Vec<_>>());
    let output_ext =
        compatible_merge_ext(&vcodecs, &acodecs, &vexts, &aexts, preferences.as_deref());
    let the_only_video = (vcodecs.len() == 1)
        .then(|| {
            parts
                .iter()
                .find(|format| format_has_video(format))
                .cloned()
        })
        .flatten();
    let the_only_audio = (acodecs.len() == 1)
        .then(|| {
            parts
                .iter()
                .find(|format| format_has_audio(format))
                .cloned()
        })
        .flatten();
    let format_label = ordered_set(
        parts
            .iter()
            .map(|format| {
                format
                    .get("format")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| format.get("format_id").and_then(serde_json::Value::as_str))
                    .unwrap_or("")
                    .to_owned()
            })
            .filter(|label| !label.is_empty())
            .collect(),
    )
    .join("+");
    let language = ordered_set(
        parts
            .iter()
            .filter_map(|format| {
                format
                    .get("language")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .collect(),
    );
    let format_note = ordered_set(
        parts
            .iter()
            .filter_map(|format| {
                format
                    .get("format_note")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .collect(),
    );
    // `sum(...) or None`: a zero total collapses to no estimate.
    let filesize_approx = parts
        .iter()
        .filter_map(|format| {
            first_present(format, &["filesize", "filesize_approx"])
                .and_then(serde_json::Value::as_f64)
        })
        .sum::<f64>();
    let filesize_approx = if filesize_approx == 0.0 {
        None
    } else {
        Some(filesize_approx)
    };
    let tbr = parts
        .iter()
        .filter_map(|format| {
            first_present(format, &["tbr", "vbr", "abr"]).and_then(|value| match value {
                serde_json::Value::Number(value) => value.as_f64(),
                serde_json::Value::String(value) => value.parse::<f64>().ok(),
                _ => None,
            })
        })
        .sum::<f64>();
    let mut merged = serde_json::Map::new();
    merged.insert(
        "requested_formats".to_owned(),
        serde_json::Value::Array(parts.clone()),
    );
    merged.insert("format".to_owned(), serde_json::json!(format_label));
    merged.insert(
        "format_id".to_owned(),
        serde_json::json!(parts
            .iter()
            .filter_map(|format| format.get("format_id").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join("+")),
    );
    merged.insert("ext".to_owned(), serde_json::json!(output_ext));
    merged.insert(
        "protocol".to_owned(),
        serde_json::json!(parts
            .iter()
            .map(format_protocol)
            .collect::<Vec<_>>()
            .join("+")),
    );
    merged.insert(
        "language".to_owned(),
        language
            .first()
            .map(|language| serde_json::json!(language))
            .unwrap_or(serde_json::Value::Null),
    );
    merged.insert(
        "format_note".to_owned(),
        if format_note.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::json!(format_note.join(", "))
        },
    );
    merged.insert(
        "filesize_approx".to_owned(),
        filesize_approx
            .map(|size| serde_json::json!(size))
            .unwrap_or(serde_json::Value::Null),
    );
    merged.insert("tbr".to_owned(), serde_json::json!(tbr));
    if let Some(video) = the_only_video.as_ref() {
        for field in [
            "width",
            "height",
            "resolution",
            "fps",
            "vcodec",
            "vbr",
            "stretched_ratio",
            "aspect_ratio",
            "dynamic_range",
        ] {
            merged.insert(
                field.to_owned(),
                video.get(field).cloned().unwrap_or(serde_json::Value::Null),
            );
        }
    }
    if let Some(audio) = the_only_audio.as_ref() {
        for field in ["acodec", "abr", "asr", "audio_channels"] {
            merged.insert(
                field.to_owned(),
                audio.get(field).cloned().unwrap_or(serde_json::Value::Null),
            );
        }
    }
    serde_json::Value::Object(merged)
}

/// Evaluate a parsed selector against sorted formats, mirroring
/// `_build_selector_function`. `MERGE` yields one virtual format per
/// combination; `PICKFIRST` takes the first non-empty branch.
fn eval_selector(
    selector: &FormatSelector,
    formats: &[serde_json::Value],
    ctx: &SelectionContext,
    merge_output_format: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    match selector {
        FormatSelector::Single { spec, filters } => {
            let mut candidates = eval_atom(spec, formats, ctx, merge_output_format);
            for filter_spec in filters {
                let filter = build_format_filter(filter_spec)
                    .map_err(|message| format!("Invalid filter {filter_spec:?}: {message}"))?;
                candidates.retain(|format| format_matches_filter(format, &filter));
            }
            // Best/worst atoms keep their `.N`-th match after filtering.
            if let Some(index) = best_worst_pick_index(spec) {
                candidates = candidates
                    .into_iter()
                    .skip(index.saturating_sub(1))
                    .take(1)
                    .collect();
            }
            Ok(candidates)
        }
        FormatSelector::PickFirst(first, second) => {
            let primary = eval_selector(first, formats, ctx, merge_output_format)?;
            if primary.is_empty() {
                eval_selector(second, formats, ctx, merge_output_format)
            } else {
                Ok(primary)
            }
        }
        FormatSelector::Merge(first, second) => {
            let mut merged = Vec::new();
            for left in eval_selector(first, formats, ctx, merge_output_format)? {
                for right in eval_selector(second, formats, ctx, merge_output_format)? {
                    merged.push(merge_format_pair(&left, &right, merge_output_format));
                }
            }
            Ok(merged)
        }
        FormatSelector::Group(selectors) | FormatSelector::Concat(selectors) => {
            let mut combined = Vec::new();
            for selector in selectors {
                combined.extend(eval_selector(selector, formats, ctx, merge_output_format)?);
            }
            Ok(combined)
        }
    }
}

/// Whether an ffmpeg executable is available for merging, mirroring the
/// default-format evaluation (`FFmpegMergerPP.available`).
fn native_ffmpeg_available(options: &cli::CliOptions) -> bool {
    if let Some(location) = options.ffmpeg_location.as_deref() {
        if !location.trim().is_empty() {
            return true;
        }
    }
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| {
            ["ffmpeg", "ffmpeg.exe"]
                .iter()
                .any(|name| directory.join(name).is_file())
        })
    })
}

/// Select the downloadable formats for an info dict, mirroring
/// `process_video_result`'s format-selection stage: sort worst-first, apply
/// the `-f` selector (or the default), and return one entry per format to
/// download. Merged entries carry `requested_formats`.
fn select_native_downloads(
    info: &InfoDict,
    selector: Option<&str>,
    options: &cli::CliOptions,
) -> Result<Vec<NativeSelection>, String> {
    let mut formats = match info.get("formats").and_then(serde_json::Value::as_array) {
        Some(formats) => formats.clone(),
        // Like `_get_formats`, a bare URL acts as its own single format.
        None => match info.get("url").and_then(serde_json::Value::as_str) {
            Some(_)
                if info.get("_type").and_then(serde_json::Value::as_str) != Some("playlist") =>
            {
                vec![serde_json::Value::Object(
                    info.iter()
                        .map(|(key, value)| (key.to_owned(), value.clone()))
                        .collect(),
                )]
            }
            _ => Vec::new(),
        },
    };
    // Malformed formats without a URL never participate in selection.
    formats.retain(|format| {
        format
            .get("url")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|url| !url.is_empty())
    });
    let extractor_sorts = info
        .get("_format_sort_fields")
        .and_then(serde_json::Value::as_array)
        .map(|fields| {
            fields
                .iter()
                .filter_map(|field| field.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    sort_native_formats(&mut formats, &options.format_sort, &extractor_sorts);
    // Mirrors the `process_video_result` DRM pass: `_has_drm` is set when any
    // format is definitely DRM (truthy `has_drm` other than `'maybe'`), and
    // those formats are dropped unless `--allow-unplayable-formats` is given.
    // Test-download probing (`--check-formats`) stays TODO: untestable
    // formats are excluded instead of probed.
    let has_drm = formats
        .iter()
        .any(|format| format_drm_state(format) == FormatDrmState::Blocked);
    if !options.allow_unplayable_formats {
        formats.retain(|format| format_drm_state(format) != FormatDrmState::Blocked);
    }
    if formats.is_empty() && has_drm {
        // Mirrors `raise_no_formats` with `_has_drm` set.
        return Err("This video is DRM protected".to_owned());
    }
    let spec = match selector {
        Some(spec) => spec.to_owned(),
        None if options.extractaudio => "bestaudio".to_owned(),
        // The default download format, like the oracle: merge when ffmpeg
        // is available, otherwise the best single format.
        None if native_ffmpeg_available(options) => "bv*+ba/b".to_owned(),
        None => {
            eprintln!(
                "[warning] ffmpeg not found; falling back to best single format \
                 (install ffmpeg for bestvideo+bestaudio merges)"
            );
            "best".to_owned()
        }
    };
    let parsed = parse_format_selector(&spec)?;
    let ctx = selection_context(&formats);
    let mut selections = Vec::new();
    for selector in &parsed {
        for format in eval_selector(
            selector,
            &formats,
            &ctx,
            options.merge_output_format.as_deref(),
        )? {
            if format
                .get("requested_formats")
                .and_then(serde_json::Value::as_array)
                .is_some()
            {
                selections.push(NativeSelection::Merged(format));
            } else {
                selections.push(NativeSelection::Single(format));
            }
        }
    }
    Ok(selections)
}

/// The DRM state of one format, mirroring the `has_drm` truthiness checks in
/// `process_video_result` and `list_formats`: the `'maybe'` string (used for
/// HLS formats whose DRM status is unknown) stays selectable with a warning
/// marker, while any other truthy `has_drm` is blocked by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormatDrmState {
    Free,
    Maybe,
    Blocked,
}

pub(crate) fn format_drm_state(format: &serde_json::Value) -> FormatDrmState {
    match format.get("has_drm") {
        None | Some(serde_json::Value::Null) => FormatDrmState::Free,
        Some(serde_json::Value::String(marker)) if marker == "maybe" => FormatDrmState::Maybe,
        Some(serde_json::Value::Bool(false)) => FormatDrmState::Free,
        Some(serde_json::Value::String(marker)) if marker.is_empty() => FormatDrmState::Free,
        Some(serde_json::Value::Number(count))
            if count.as_i64().is_some_and(|count| count == 0)
                || count.as_f64().is_some_and(|count| count == 0.0) =>
        {
            FormatDrmState::Free
        }
        Some(serde_json::Value::Array(items)) if items.is_empty() => FormatDrmState::Free,
        Some(serde_json::Value::Object(fields)) if fields.is_empty() => FormatDrmState::Free,
        _ => FormatDrmState::Blocked,
    }
}

/// Print the formats table exactly like `--list-formats`: sorted first,
/// mirroring the oracle's table pass before interactive selection.
pub(crate) fn print_sorted_formats(info: &InfoDict, options: &cli::CliOptions) {
    let mut view = info.clone();
    if let Some(formats) = view
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .cloned()
    {
        let mut formats = formats;
        sort_native_formats(&mut formats, &options.format_sort, &[]);
        view.insert("formats", serde_json::Value::Array(formats));
    }
    print_formats(&view);
}

/// Run the interactive `-f -` selection loop, mirroring `process_video_result`:
/// the table is printed by the caller, then each reply is parsed as a format
/// selector. An empty reply selects the default spec; syntax errors and empty
/// matches report `ERROR: ...` and reprompt. With no formats at all the loop
/// is skipped for the same "Requested format is not available" error the
/// non-interactive path raises. A closed stdin aborts cleanly instead of
/// raising Python's EOFError.
pub(crate) fn select_interactive_downloads(
    info: &InfoDict,
    options: &cli::CliOptions,
    prompt: &dyn Fn(),
    read_reply: &mut dyn FnMut() -> Option<String>,
) -> Result<Vec<NativeSelection>, String> {
    let any_formats = info
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|formats| !formats.is_empty())
        || info
            .get("url")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|url| !url.is_empty());
    if !any_formats {
        return Err("Requested format is not available".to_owned());
    }
    loop {
        prompt();
        let Some(reply) = read_reply() else {
            return Err("format selection aborted: standard input is closed".to_owned());
        };
        let selector = if reply.is_empty() {
            None
        } else {
            Some(reply.as_str())
        };
        match select_native_downloads(info, selector, options) {
            Ok(selections) if !selections.is_empty() => return Ok(selections),
            Ok(_) => eprintln!("ERROR: Requested format is not available"),
            Err(error) => eprintln!("ERROR: {error}"),
        }
    }
}

#[cfg(test)]
mod interactive_format_tests {
    use super::*;

    fn two_format_info() -> InfoDict {
        let mut info = InfoDict::new();
        info.insert(
            "formats",
            serde_json::json!([
                {"format_id": "a1", "ext": "mp4", "vcodec": "avc1", "acodec": "none",
                 "resolution": "640x360", "url": "https://media.test/a1.mp4"},
                {"format_id": "a2", "ext": "mp4", "vcodec": "avc1", "acodec": "none",
                 "resolution": "1280x720", "url": "https://media.test/a2.mp4"},
            ]),
        );
        info
    }

    fn scripted_run(
        info: &InfoDict,
        options: &cli::CliOptions,
        replies: Vec<&str>,
    ) -> (usize, Result<Vec<String>, String>) {
        let prompts = std::cell::Cell::new(0);
        let mut answers = replies;
        answers.reverse();
        let outcome = select_interactive_downloads(
            info,
            options,
            &|| prompts.set(prompts.get() + 1),
            &mut || answers.pop().map(str::to_owned),
        );
        let ids = outcome.map(|selections| {
            selections
                .iter()
                .map(|selection| match selection {
                    NativeSelection::Single(format) | NativeSelection::Merged(format) => format
                        .get("format_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                })
                .collect()
        });
        (prompts.get(), ids)
    }

    #[test]
    fn interactive_selection_reprompts_until_a_reply_matches() {
        // Bad syntax, then an unknown ID, then a direct pick: three prompts.
        let (prompts, ids) = scripted_run(
            &two_format_info(),
            &cli::CliOptions::default(),
            vec!["oops[", "zzz", "a2"],
        );
        assert_eq!(prompts, 3);
        assert_eq!(ids.unwrap(), vec!["a2".to_owned()]);
    }

    #[test]
    fn interactive_selection_empty_reply_uses_default_spec() {
        let mut single = InfoDict::new();
        single.insert(
            "formats",
            serde_json::json!([
                {"format_id": "only", "ext": "mp4", "vcodec": "avc1",
                 "url": "https://media.test/only.mp4"},
            ]),
        );
        let (prompts, ids) = scripted_run(&single, &cli::CliOptions::default(), vec![""]);
        assert_eq!(prompts, 1);
        assert_eq!(ids.unwrap(), vec!["only".to_owned()]);
    }

    #[test]
    fn interactive_selection_errors_without_formats_or_replies() {
        let options = cli::CliOptions::default();
        // No formats at all: no prompt, straight to the unavailable error.
        let (prompts, outcome) = scripted_run(&InfoDict::new(), &options, vec!["a1"]);
        assert_eq!(prompts, 0);
        assert_eq!(outcome.unwrap_err(), "Requested format is not available");
        // Closed stdin aborts instead of hanging or panicking.
        let (prompts, outcome) = scripted_run(&two_format_info(), &options, vec![]);
        assert_eq!(prompts, 1);
        assert_eq!(
            outcome.unwrap_err(),
            "format selection aborted: standard input is closed"
        );
    }
}

#[cfg(test)]
mod drm_probing_tests {
    use super::*;

    /// One AV format with the given id, height, and DRM state.
    fn av_format(id: &str, height: u64, drm: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "format_id": id, "ext": "mp4", "vcodec": "avc1", "acodec": "mp4a",
            "url": format!("https://media.test/{id}.mp4"), "height": height, "has_drm": drm,
        })
    }

    fn mixed_drm_info() -> InfoDict {
        let mut info = InfoDict::new();
        info.insert(
            "formats",
            serde_json::json!([
                av_format("drm1080", 1080, serde_json::Value::Bool(true)),
                av_format("free720", 720, serde_json::Value::Bool(false)),
            ]),
        );
        info
    }

    /// Selected format ids in selection order.
    fn selected_ids(
        info: &InfoDict,
        spec: Option<&str>,
        options: &cli::CliOptions,
    ) -> Result<Vec<String>, String> {
        let selection = select_native_downloads(info, spec, options)?;
        Ok(selection
            .iter()
            .map(|item| match item {
                NativeSelection::Single(format) | NativeSelection::Merged(format) => format
                    .get("format_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?")
                    .to_owned(),
            })
            .collect())
    }

    #[test]
    fn drm_formats_filtered_unless_allowed() {
        let info = mixed_drm_info();
        // Deterministic spec: without the flag the protected stream is filtered.
        assert_eq!(
            selected_ids(&info, Some("best"), &cli::CliOptions::default()).unwrap(),
            vec!["free720".to_owned()]
        );
        // With the flag the protected stream is selectable and wins as best.
        let mut allowed = cli::CliOptions::default();
        allowed.allow_unplayable_formats = true;
        assert_eq!(
            selected_ids(&info, Some("best"), &allowed).unwrap(),
            vec!["drm1080".to_owned()]
        );
    }

    #[test]
    fn maybe_drm_stays_selectable_without_flag() {
        let mut info = InfoDict::new();
        info.insert(
            "formats",
            serde_json::json!([
                av_format("maybe1080", 1080, serde_json::json!("maybe")),
                av_format("free720", 720, serde_json::Value::Null),
            ]),
        );
        // "maybe" is not dropped either way.
        assert_eq!(
            selected_ids(&info, Some("best"), &cli::CliOptions::default()).unwrap(),
            vec!["maybe1080".to_owned()]
        );
    }

    #[test]
    fn all_drm_errors_protected_unless_allowed() {
        let mut info = InfoDict::new();
        info.insert(
            "formats",
            serde_json::json!([av_format("drm720", 720, serde_json::Value::Bool(true))]),
        );
        // Exact `raise_no_formats` message from the extractor oracle.
        assert_eq!(
            selected_ids(&info, Some("best"), &cli::CliOptions::default()).unwrap_err(),
            "This video is DRM protected"
        );
        let mut allowed = cli::CliOptions::default();
        allowed.allow_unplayable_formats = true;
        assert_eq!(
            selected_ids(&info, Some("best"), &allowed).unwrap(),
            vec!["drm720".to_owned()]
        );
    }

    #[test]
    fn drm_state_truthiness_battery() {
        let state = |value: serde_json::Value| {
            format_drm_state(&serde_json::json!({"format_id": "x", "has_drm": value}))
        };
        // Exact post-filter truth table.
        assert_eq!(
            state(serde_json::Value::Bool(true)),
            FormatDrmState::Blocked
        );
        assert_eq!(state(serde_json::json!("maybe")), FormatDrmState::Maybe);
        assert_eq!(state(serde_json::Value::Bool(false)), FormatDrmState::Free);
        assert_eq!(state(serde_json::Value::Null), FormatDrmState::Free);
        assert_eq!(state(serde_json::json!(0)), FormatDrmState::Free);
        assert_eq!(state(serde_json::json!("")), FormatDrmState::Free);
        // Other truthy values are treated as DRM.
        assert_eq!(state(serde_json::json!(1)), FormatDrmState::Blocked);
        assert_eq!(state(serde_json::json!("yes")), FormatDrmState::Blocked);
        // Missing key is free.
        assert_eq!(
            format_drm_state(&serde_json::json!({"format_id": "x"})),
            FormatDrmState::Free
        );
    }
}
