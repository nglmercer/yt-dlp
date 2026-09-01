fn mangomolo_base64_decode(value: &str) -> Option<Vec<u8>> {
    let mut accumulator = 0u32;
    let mut bit_count = 0u8;
    let mut decoded = Vec::with_capacity(value.len() * 3 / 4);
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
        accumulator = (accumulator << 6) | u32::from(digit);
        bit_count = bit_count.saturating_add(6);
        if bit_count >= 8 {
            bit_count -= 8;
            decoded.push(((accumulator >> bit_count) & 0xff) as u8);
        }
    }
    Some(decoded)
}

fn mangomolo_live_id(page_id: &str) -> Option<String> {
    let decoded = percent_decode(page_id);
    let bytes = mangomolo_base64_decode(&decoded)?;
    String::from_utf8(bytes).ok().filter(|value| !value.is_empty())
}

fn mangomolo_player_url(url: &str, player_type: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    Some(format!("https://player.mangomolo.com/v1/{player_type}?{query}"))
}
