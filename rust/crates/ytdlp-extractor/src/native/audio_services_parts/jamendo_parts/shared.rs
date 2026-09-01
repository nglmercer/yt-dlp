const JAMENDO_API_BASE: &str = "https://www.jamendo.com";

fn jamendo_nonce() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("0.{:016}", nanos % 10_000_000_000_000_000_u128)
}

pub(crate) fn jamendo_sha1_hex(input: &[u8]) -> String {
    let mut message = input.to_vec();
    let bit_length = (message.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = [
        0x6745_2301_u32,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    for chunk in message.chunks_exact(64) {
        let mut words = [0_u32; 80];
        for (index, bytes) in chunk.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for index in 16..80 {
            words[index] = (words[index - 3]
                ^ words[index - 8]
                ^ words[index - 14]
                ^ words[index - 16])
                .rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (index, word) in words.iter().enumerate() {
            let (function, constant) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temporary = a
                .rotate_left(5)
                .wrapping_add(function)
                .wrapping_add(e)
                .wrapping_add(constant)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temporary;
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
    }
    state
        .iter()
        .map(|word| format!("{word:08x}"))
        .collect::<String>()
}

fn jamendo_call_api(
    context: &ExtractionContext,
    resource: &str,
    resource_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let path = format!("/api/{resource}s");
    let nonce = jamendo_nonce();
    let signature = jamendo_sha1_hex(format!("{path}{nonce}").as_bytes());
    let mut request = Request::new(format!("{JAMENDO_API_BASE}{path}"));
    request.update_query(&[("id[]".to_owned(), resource_id.to_owned())]);
    request
        .headers_mut()
        .set("X-Jam-Call", format!("${signature}*{nonce}~"));
    let response = context.request(&request)?;
    let values = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Jamendo {resource} JSON for {resource_id}: {error}"),
        )
    })?;
    values
        .as_array()
        .and_then(|values| values.first())
        .cloned()
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Jamendo {resource} response for {resource_id} is empty"),
            )
        })
}

fn jamendo_optional_api(
    context: &ExtractionContext,
    resource: &str,
    resource_id: Option<&str>,
) -> Option<serde_json::Value> {
    resource_id.and_then(|resource_id| jamendo_call_api(context, resource, resource_id).ok())
}

fn jamendo_integer(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_f64().map(|value| value as i64))
            .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
    })
}

fn jamendo_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .filter(|value| !value.is_empty())
}

fn jamendo_upload_date(timestamp: i64) -> String {
    // Proleptic Gregorian conversion using whole UTC days. The API timestamp
    // is UTC, and yt-dlp's upload_date is the UTC calendar date.
    let days = timestamp.div_euclid(86_400);
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }).div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era
        - day_of_era.div_euclid(1_460)
        + day_of_era.div_euclid(36_524)
        - day_of_era.div_euclid(146_096))
        .div_euclid(365);
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era
        - (365 * year_of_era + year_of_era.div_euclid(4) - year_of_era.div_euclid(100));
    let month_part = (5 * day_of_year + 2).div_euclid(153);
    let day = day_of_year - (153 * month_part + 2).div_euclid(5) + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}")
}

fn jamendo_thumbnail_extension(response: &yt_dlp_networking::Response, cover_url: &str) -> String {
    response
        .headers()
        .get("Content-Type")
        .and_then(|value| value.split(';').next())
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "image/jpeg" | "image/jpg" => Some("jpg"),
            "image/png" => Some("png"),
            "image/webp" => Some("webp"),
            "image/gif" => Some("gif"),
            _ => None,
        })
        .map(str::to_owned)
        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(cover_url), "jpg"))
}
