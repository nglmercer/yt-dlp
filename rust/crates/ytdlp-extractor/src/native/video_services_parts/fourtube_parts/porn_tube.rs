fn fourtube_porn_tube_video(
    html: &str,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let encoded = Regex::new(
        r#"(?is)\bINITIALSTATE\s*=\s*(["'])(?P<value>(?:(?!\1).)+)\1"#,
    )
    .ok()
    .and_then(|matcher| matcher.captures(html).ok().flatten())
    .and_then(|captures| captures.name("value"))
    .map(|value| value.as_str().to_owned())
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: PornTube video {video_id} has no INITIALSTATE payload"),
        )
    })?;
    let decoded = fourtube_base64_decode(&encoded)
        .and_then(|value| String::from_utf8(value).ok())
        .map(|value| percent_decode(&value))
        .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
        .and_then(|value| value.get("page").and_then(|page| page.get("video")).cloned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: PornTube video {video_id} has an unsupported INITIALSTATE payload"
                ),
            )
        })?;
    Ok(decoded)
}

fn fourtube_base64_decode(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in value.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            break;
        }
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(digit);
        bits = bits.saturating_add(6);
        while bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            if bits > 0 {
                buffer &= (1 << bits) - 1;
            } else {
                buffer = 0;
            }
        }
    }
    Some(output)
}

fn chrono_like_date_digits(timestamp: i64) -> Option<String> {
    let days = timestamp.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days)?;
    Some(format!("{year:04}{month:02}{day:02}"))
}

fn civil_from_days(days: i64) -> Option<(i64, i64, i64)> {
    let z = days.checked_add(719_468)?;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year >= 0).then_some((year, month, day))
}
