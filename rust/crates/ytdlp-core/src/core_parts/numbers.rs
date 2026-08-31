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
