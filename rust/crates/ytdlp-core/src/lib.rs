//! Foundational types for the experimental Rust migration.
//!
//! This crate deliberately starts with a dynamic info dictionary. Extractors
//! in yt-dlp add service-specific fields, so a fixed Rust struct would lose
//! information and break compatibility.

use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::LazyLock;

pub const MIGRATION_VERSION: &str = "0.0.0";

const BYTE_SUFFIXES: [&str; 9] = ["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"];
static PARSE_BYTES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<num>\d+(?:\.\d+)?)\s*(?P<unit>[KMGTPEZY]?)$").unwrap());
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

/// Backend used for a capability while the migration is in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Rust,
    PythonCompatibility,
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

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_map(&self) -> &IndexMap<String, Value> {
        &self.0
    }

    pub fn into_map(self) -> IndexMap<String, Value> {
        self.0
    }
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

/// Capabilities available in the first scaffold. No production feature is
/// claimed until it has a differential test against the Python reference.
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
}
