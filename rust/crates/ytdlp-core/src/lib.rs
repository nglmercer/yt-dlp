//! Foundational types for the experimental Rust migration.
//!
//! This crate deliberately starts with a dynamic info dictionary. Extractors
//! add service-specific fields, so a fixed Rust struct would lose information
//! and break behavioral parity.

use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::sync::LazyLock;

mod archive;

pub use archive::{ArchiveError, DownloadArchive};

pub const MIGRATION_VERSION: &str = "0.0.0";

const BYTE_SUFFIXES: [&str; 9] = ["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"];
static PARSE_BYTES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<num>\d+(?:\.\d+)?)\s*(?P<unit>[KMGTPEZY]?)$").unwrap());
static OUTPUT_TEMPLATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"%\((?P<key>[^)]+)\)(?P<format>[#0\-+ ]?\d*(?:\.\d+)?[sdif])").unwrap()
});
static DURATION_CLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:(?:(?P<days>\d+):)?(?P<hours>\d+):)?(?P<mins>\d+):(?P<secs>\d{1,2})(?P<ms>[.:]\d+)?Z?$",
    )
    .unwrap()
});
static DURATION_SECONDS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<secs>\d+)(?P<ms>[.:]\d+)?Z?$").unwrap());
static DURATION_UNITS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)^(?:P?
        (?:\d+\s*y(?:ears?)?,?\s*)?
        (?:\d+\s*m(?:onths?)?,?\s*)?
        (?:\d+\s*w(?:eeks?)?,?\s*)?
        (?:(?P<days>\d+)\s*d(?:ays?)?,?\s*)?
        T)?
        (?:(?P<hours>\d+)\s*h(?:(?:ou)?rs?)?,?\s*)?
        (?:(?P<mins>\d+)\s*m(?:in(?:ute)?s?)?,?\s*)?
        (?:(?P<secs>\d+)(?P<ms>\.\d+)?\s*s(?:ec(?:ond)?s?)?\s*)?
        Z?$",
    )
    .unwrap()
});
static DURATION_TEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)^(?:(?P<hours>[0-9.]+)\s*(?:hours?)|(?P<mins>[0-9.]+)\s*(?:mins?\.?|minutes?)\s*)Z?$",
    )
    .unwrap()
});
static ISO8601_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})T(?P<hour>\d{2}):(?P<minute>\d{2}):(?P<second>\d{2})(?:\.\d+)?(?P<timezone>Z|(?P<sign>[+-])(?P<tzhour>\d{2}):?(?P<tzminute>\d{2}))?$",
    )
    .unwrap()
});
static URL_SCHEME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+.-]*:").unwrap());

/// Implementation state used by the migration matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Rust,
    Todo,
}

/// A capability entry used by the migration matrix and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    pub name: &'static str,
    pub mode: EngineMode,
}

/// A JSON-compatible, insertion-order-preserving info dictionary.
///
/// This is intentionally a transitional representation. Non-JSON Python
/// values and lazy entries will need explicit internal variants before the
/// Rust engine can claim full embedding/API parity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InfoDict(IndexMap<String, Value>);

impl InfoDict {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: Value) -> Option<Value> {
        self.0.insert(key.into(), value)
    }

    pub fn insert_if_some<T>(&mut self, key: impl Into<String>, value: Option<T>)
    where
        T: Serialize,
    {
        if let Some(value) = value {
            self.insert(key, serde_json::to_value(value).unwrap_or(Value::Null));
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.0.shift_remove(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        })
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    pub fn as_map(&self) -> &IndexMap<String, Value> {
        &self.0
    }

    pub fn into_map(self) -> IndexMap<String, Value> {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreErrorKind {
    InvalidInput,
    Unsupported,
    MissingField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreError {
    pub kind: CoreErrorKind,
    pub message: String,
}

impl CoreError {
    pub fn new(kind: CoreErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for CoreError {}

fn render_template_value(value: &Value, format_spec: &str) -> Result<String, CoreError> {
    let conversion = format_spec.chars().last().ok_or_else(|| {
        CoreError::new(CoreErrorKind::InvalidInput, "empty output template format")
    })?;
    let modifiers = &format_spec[..format_spec.len() - conversion.len_utf8()];
    let zero_padded = modifiers.contains('0');
    let width = modifiers
        .trim_start_matches(['#', '0', '-', '+', ' '])
        .split_once('.')
        .map_or(
            modifiers.trim_start_matches(['#', '0', '-', '+', ' ']),
            |(width, _)| width,
        )
        .parse::<usize>()
        .unwrap_or(0);
    let precision = modifiers
        .split_once('.')
        .and_then(|(_, precision)| precision.parse::<usize>().ok());
    let rendered = match conversion {
        's' => match value {
            Value::String(value) => value.clone(),
            Value::Null => String::new(),
            value => value.to_string(),
        },
        'd' | 'i' => {
            let integer = value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
                .or_else(|| value.as_f64().map(|value| value as i64))
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
                .ok_or_else(|| {
                    CoreError::new(
                        CoreErrorKind::InvalidInput,
                        format!("value {value} is not an integer"),
                    )
                })?;
            integer.to_string()
        }
        'f' => {
            let number = value
                .as_f64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
                .ok_or_else(|| {
                    CoreError::new(
                        CoreErrorKind::InvalidInput,
                        format!("value {value} is not a number"),
                    )
                })?;
            precision.map_or_else(
                || number.to_string(),
                |precision| format!("{number:.precision$}"),
            )
        }
        _ => {
            return Err(CoreError::new(
                CoreErrorKind::InvalidInput,
                format!("unsupported output template format: {format_spec}"),
            ));
        }
    };
    if width <= rendered.len() {
        return Ok(rendered);
    }
    let padding = width - rendered.len();
    if zero_padded && conversion != 's' {
        Ok(format!("{}{}", "0".repeat(padding), rendered))
    } else {
        Ok(format!("{}{}", " ".repeat(padding), rendered))
    }
}

/// Render the initial Python-style output-template subset used by the native
/// downloader. Unknown fields and unsupported conversions fail explicitly.
pub fn render_output_template(template: &str, info: &InfoDict) -> Result<String, CoreError> {
    let mut output = String::new();
    let mut end = 0;
    for captures in OUTPUT_TEMPLATE_RE.captures_iter(template) {
        let whole = captures.get(0).expect("regex capture 0");
        output.push_str(&template[end..whole.start()]);
        let key = captures.name("key").expect("output key").as_str();
        let value = info.get(key).ok_or_else(|| {
            CoreError::new(
                CoreErrorKind::MissingField,
                format!("output template field is missing: {key}"),
            )
        })?;
        output.push_str(&render_template_value(
            value,
            captures.name("format").expect("output format").as_str(),
        )?);
        end = whole.end();
    }
    output.push_str(&template[end..]);
    Ok(output.replace("%%", "%"))
}

/// Format a byte count using yt-dlp's binary suffixes.
///
/// The exponent is selected by repeated division instead of a logarithm so
/// exact powers of 1024 remain on the same boundary as the Python reference.
pub fn format_bytes(bytes: Option<f64>) -> String {
    let Some(bytes) = bytes else {
        return "N/A".to_owned();
    };

    if !bytes.is_finite() || bytes < 0.0 {
        return "N/A".to_owned();
    }

    let mut exponent = 0;
    let mut converted = bytes;
    while exponent + 1 < BYTE_SUFFIXES.len() && converted >= 1024.0 {
        converted /= 1024.0;
        exponent += 1;
    }

    format!("{converted:.2}{}B", BYTE_SUFFIXES[exponent])
}

/// Parse a strict binary byte quantity such as `1.5K`.
///
/// yt-dlp uses Python's floating-point conversion and round-to-even behavior
/// here. The Rust implementation keeps that behavior for accepted inputs and
/// returns `None` for malformed values.
pub fn parse_bytes(input: &str) -> Option<u128> {
    let upper = input.to_uppercase();
    let captures = PARSE_BYTES_RE.captures(&upper)?;
    let number = captures.name("num")?.as_str().parse::<f64>().ok()?;
    let exponent = match captures.name("unit")?.as_str() {
        "" => 0,
        "K" => 1,
        "M" => 2,
        "G" => 3,
        "T" => 4,
        "P" => 5,
        "E" => 6,
        "Z" => 7,
        "Y" => 8,
        _ => return None,
    };
    let value = number * 1024_f64.powi(exponent);
    if !value.is_finite() {
        return None;
    }

    let floor = value.floor();
    let fraction = value - floor;
    let rounded = if fraction < 0.5 {
        floor
    } else if fraction > 0.5 || (floor as u128) % 2 == 1 {
        floor + 1.0
    } else {
        floor
    };
    Some(rounded as u128)
}

fn duration_part(captures: &regex::Captures<'_>, name: &str) -> Option<f64> {
    captures
        .name(name)
        .and_then(|value| value.as_str().replace(':', ".").parse::<f64>().ok())
}

fn duration_total(captures: &regex::Captures<'_>) -> Option<f64> {
    let values = [
        (duration_part(captures, "days"), 86_400.0),
        (duration_part(captures, "hours"), 3_600.0),
        (duration_part(captures, "mins"), 60.0),
        (duration_part(captures, "secs"), 1.0),
        (duration_part(captures, "ms"), 1.0),
    ];
    let total = values
        .into_iter()
        .map(|(value, multiplier)| value.unwrap_or(0.0) * multiplier)
        .sum::<f64>();
    total.is_finite().then_some(total)
}

/// Parse the duration forms accepted by yt-dlp.
pub fn parse_duration(input: &str) -> Option<f64> {
    if input.trim().is_empty() {
        return None;
    }
    let input = input.trim();
    for matcher in [
        &*DURATION_CLOCK_RE,
        &*DURATION_SECONDS_RE,
        &*DURATION_UNITS_RE,
        &*DURATION_TEXT_RE,
    ] {
        if let Some(captures) = matcher.captures(input) {
            return duration_total(&captures);
        }
    }
    None
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
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

/// Parse the ISO-8601 timestamp form used by extractor APIs into Unix seconds.
/// Fractional seconds are discarded to match yt-dlp's parse_iso8601 utility.
pub fn parse_iso8601(input: &str) -> Option<i64> {
    let captures = ISO8601_RE.captures(input.trim())?;
    let year = captures.name("year")?.as_str().parse::<i64>().ok()?;
    let month = captures.name("month")?.as_str().parse::<i64>().ok()?;
    let day = captures.name("day")?.as_str().parse::<i64>().ok()?;
    let hour = captures.name("hour")?.as_str().parse::<i64>().ok()?;
    let minute = captures.name("minute")?.as_str().parse::<i64>().ok()?;
    let second = captures.name("second")?.as_str().parse::<i64>().ok()?;
    if !(1..=12).contains(&month)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        return None;
    }
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        2 if leap_year => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=days_in_month).contains(&day) {
        return None;
    }
    let offset = match captures.name("sign") {
        Some(sign) => {
            let hours = captures
                .name("tzhour")
                .and_then(|value| value.as_str().parse::<i64>().ok())?;
            let minutes = captures
                .name("tzminute")
                .and_then(|value| value.as_str().parse::<i64>().ok())?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            let seconds = hours * 3_600 + minutes * 60;
            if sign.as_str() == "+" {
                seconds
            } else {
                -seconds
            }
        }
        None => 0,
    };
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second - offset)
}

/// Determine a URL's extension using the same conservative rules as
/// yt-dlp's utility function. Query strings are excluded, while a trailing
/// slash is accepted for known extension values such as `mp4/`.
pub fn determine_ext(url: Option<&str>, default_ext: &str) -> String {
    let Some(url) = url else {
        return default_ext.to_owned();
    };
    let path = url.split_once('?').map_or(url, |(path, _)| path);
    let Some((_, guess)) = path.rsplit_once('.') else {
        return default_ext.to_owned();
    };
    if !guess.is_empty() && guess.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return guess.to_owned();
    }
    let trimmed = guess.trim_end_matches('/');
    if !trimmed.is_empty()
        && matches!(
            trimmed.to_ascii_lowercase().as_str(),
            "3gp"
                | "aac"
                | "ass"
                | "avi"
                | "flac"
                | "flv"
                | "m4a"
                | "m4v"
                | "mkv"
                | "mov"
                | "m3u8"
                | "mp3"
                | "mp4"
                | "mpeg"
                | "mpg"
                | "oga"
                | "ogg"
                | "opus"
                | "srt"
                | "ssa"
                | "ts"
                | "vtt"
                | "wav"
                | "webm"
                | "webp"
        )
    {
        return trimmed.to_owned();
    }
    default_ext.to_owned()
}

/// Determine the downloader protocol implied by an info dictionary.
pub fn determine_protocol(info: &InfoDict) -> Result<String, CoreError> {
    if let Some(protocol) = info.get_str("protocol") {
        return Ok(protocol.to_owned());
    }
    let url = info.get_str("url").ok_or_else(|| {
        CoreError::new(
            CoreErrorKind::MissingField,
            "determine_protocol requires an info_dict url",
        )
    })?;
    if url.starts_with("rtmp") {
        return Ok("rtmp".to_owned());
    }
    let extension = determine_ext(Some(url), "unknown_video").to_ascii_lowercase();
    if extension == "m3u8" {
        return Ok(if info.get_bool("is_live").unwrap_or(false) {
            "m3u8"
        } else {
            "m3u8_native"
        }
        .to_owned());
    }
    if extension == "f4m" {
        return Ok("f4m".to_owned());
    }
    Ok(URL_SCHEME_RE.find(url).map_or_else(String::new, |scheme| {
        scheme.as_str().trim_end_matches(':').to_owned()
    }))
}

/// Parse an integer-like JSON value with yt-dlp's scaling semantics.
pub fn int_or_none(
    value: Option<&Value>,
    mut scale: i64,
    mut invscale: i64,
    base: Option<u32>,
) -> Option<i64> {
    if invscale == 1 && scale < 1 {
        invscale = (1.0 / scale as f64) as i64;
        scale = 1;
    }
    if scale == 0 {
        return None;
    }
    let integer = match value? {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_f64().map(|value| value as i64)),
        Value::String(value) => base.map_or_else(
            || value.parse::<i64>().ok(),
            |base| i64::from_str_radix(value.trim(), base).ok(),
        ),
        Value::Bool(value) => Some(i64::from(*value)),
        _ => None,
    }?;
    let scaled = integer.checked_mul(invscale)?;
    let quotient = scaled / scale;
    let remainder = scaled % scale;
    if remainder != 0 && ((scaled < 0) != (scale < 0)) {
        quotient.checked_sub(1)
    } else {
        Some(quotient)
    }
}

/// Parse a float-like JSON value with yt-dlp's scaling semantics.
pub fn float_or_none(value: Option<&Value>, scale: f64, invscale: f64) -> Option<f64> {
    if scale == 0.0 {
        return None;
    }
    let value = match value? {
        Value::Number(value) => value.as_f64()?,
        Value::String(value) => value.parse::<f64>().ok()?,
        Value::Bool(value) => f64::from(*value as u8),
        _ => return None,
    };
    let result = value * invscale / scale;
    result.is_finite().then_some(result)
}

/// Convert any JSON-compatible value using Python's string conversion for
/// the common scalar values used by extractor metadata.
pub fn str_or_none(value: Option<&Value>, default: Option<&str>) -> Option<String> {
    let Some(value) = value else {
        return default.map(str::to_owned);
    };
    Some(match value {
        Value::Null => return default.map(str::to_owned),
        Value::String(value) => value.clone(),
        Value::Bool(value) => if *value { "True" } else { "False" }.to_owned(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    })
}

/// Capabilities available in the first scaffold. No production feature is
/// claimed until it has a differential test against the offline reference.
pub const INITIAL_CAPABILITIES: &[Capability] = &[
    Capability {
        name: "info-dict-foundation",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "format-bytes",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "request-model",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "request-director",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "http-handler",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "cookie-jar",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "https-proxy-handler",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "parse-duration",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "core-url-and-scalar-utilities",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "ffmpeg-postprocessor-contract",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "javascript-runtime-adapter",
        mode: EngineMode::Rust,
    },
    Capability {
        name: "extractor-registry",
        mode: EngineMode::Rust,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn info_dict_preserves_insertion_order() {
        let mut info = InfoDict::new();
        info.insert("id", json!("example"));
        info.insert("title", json!("Example"));

        assert_eq!(
            serde_json::to_string(&info).unwrap(),
            r#"{"id":"example","title":"Example"}"#
        );
    }

    #[test]
    fn info_dict_round_trips_nested_values() {
        let mut info = InfoDict::new();
        info.insert("formats", json!([{ "format_id": "best", "height": 1080 }]));

        let encoded = serde_json::to_vec(&info).unwrap();
        let decoded: InfoDict = serde_json::from_slice(&encoded).unwrap();

        assert_eq!(decoded, info);
        assert!(decoded.get("formats").is_some());
    }

    #[test]
    fn info_dict_helpers_and_output_templates_preserve_fields() {
        let mut info = InfoDict::new();
        info.insert("id", json!("abc"));
        info.insert("ext", json!("mp4"));
        info.insert("playlist_index", json!(3));
        info.insert("duration", json!(1.25));

        assert_eq!(info.get_str("id"), Some("abc"));
        assert_eq!(info.get_i64("playlist_index"), Some(3));
        assert_eq!(info.get_f64("duration"), Some(1.25));
        assert_eq!(
            render_output_template("%(playlist_index)03d-%(id)s.%(ext)s", &info).unwrap(),
            "003-abc.mp4"
        );
        assert_eq!(
            render_output_template("%(duration).2f", &info).unwrap(),
            "1.25"
        );
        assert!(matches!(
            render_output_template("%(missing)s", &info),
            Err(CoreError {
                kind: CoreErrorKind::MissingField,
                ..
            })
        ));
    }

    #[test]
    fn format_bytes_matches_reference_cases() {
        let cases = [
            (None, "N/A"),
            (Some(-1.0), "N/A"),
            (Some(-0.0), "-0.00B"),
            (Some(0.0), "0.00B"),
            (Some(1000.0), "1000.00B"),
            (Some(1024.0), "1.00KiB"),
            (Some(1024.0_f64.powi(8)), "1.00YiB"),
            (Some(1024.0_f64.powi(9)), "1024.00YiB"),
        ];

        for (input, expected) in cases {
            assert_eq!(format_bytes(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn parse_bytes_matches_reference_cases() {
        assert_eq!(parse_bytes("0"), Some(0));
        assert_eq!(parse_bytes("1.5K"), Some(1536));
        assert_eq!(parse_bytes("1Y"), Some(1024_u128.pow(8)));
        assert_eq!(parse_bytes("1,5K"), None);
        assert_eq!(parse_bytes("1KB"), None);
        assert_eq!(parse_bytes(" 1K"), None);
    }

    #[test]
    fn parse_duration_matches_reference_examples() {
        assert_eq!(parse_duration("1"), Some(1.0));
        assert_eq!(parse_duration("1337:12"), Some(80_232.0));
        assert_eq!(parse_duration("9:12:43"), Some(33_163.0));
        assert_eq!(parse_duration("3h 11m 53s"), Some(11_513.0));
        assert_eq!(parse_duration("2.5 hours"), Some(9_000.0));
        assert_eq!(parse_duration("PT1H0.040S"), Some(3_600.04));
        assert_eq!(parse_duration("01:02:03:050"), Some(3_723.05));
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn parse_iso8601_matches_utc_and_offset_examples() {
        assert_eq!(parse_iso8601("2015-04-08T00:00:00Z"), Some(1_428_451_200));
        assert_eq!(
            parse_iso8601("2015-04-08T02:00:00+02:00"),
            Some(1_428_451_200)
        );
        assert_eq!(
            parse_iso8601("2015-04-08T00:00:00-0500"),
            Some(1_428_469_200)
        );
        assert_eq!(parse_iso8601("2015-02-29T00:00:00Z"), None);
    }

    #[test]
    fn core_url_and_scalar_utilities_match_reference_examples() {
        assert_eq!(
            determine_ext(Some("https://example.test/video.mp4?download=1"), "unknown"),
            "mp4"
        );
        assert_eq!(
            determine_ext(Some("https://example.test/manifest.m3u8/"), "unknown"),
            "m3u8"
        );
        assert_eq!(determine_ext(None, "custom"), "custom");

        let mut info = InfoDict::new();
        info.insert("url", json!("https://example.test/manifest.m3u8"));
        assert_eq!(determine_protocol(&info).unwrap(), "m3u8_native");
        info.insert("is_live", json!(true));
        assert_eq!(determine_protocol(&info).unwrap(), "m3u8");
        info.insert("protocol", json!("http_dash_segments"));
        assert_eq!(determine_protocol(&info).unwrap(), "http_dash_segments");

        assert_eq!(int_or_none(Some(&json!("1536")), 1024, 1, None), Some(1));
        assert_eq!(int_or_none(Some(&json!(-3)), 2, 1, None), Some(-2));
        assert_eq!(float_or_none(Some(&json!("1.5")), 2.0, 1.0), Some(0.75));
        assert_eq!(
            str_or_none(Some(&json!(true)), None),
            Some("True".to_owned())
        );
        assert_eq!(
            str_or_none(None, Some("fallback")),
            Some("fallback".to_owned())
        );
    }
}
