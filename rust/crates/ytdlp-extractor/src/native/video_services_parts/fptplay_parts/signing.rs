const FPTPLAY_SECRET: &str = "WEBv6Dkdsad90dasdjlALDDDS";

fn fptplay_api_url(video_id: &str, episode: i64) -> String {
    let path = format!("/api/v6.2_w/stream/vod/{video_id}/{episode}/auto_vip");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
        + 10_800;
    let message = format!("{FPTPLAY_SECRET}{timestamp}{path}");
    let token = fptplay_token(&message);
    format!("https://api.fptplay.net{path}?st={token}&e={timestamp}")
}

fn fptplay_token(message: &str) -> String {
    fptplay_base64_urlsafe(&native_md5(message.as_bytes()))
}

fn fptplay_base64_urlsafe(bytes: &[u8; 16]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(22);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(ALPHABET[(first >> 2) as usize] as char);
        encoded.push(ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            encoded.push(ALPHABET[(third & 0x3f) as usize] as char);
        }
    }
    encoded.replace('+', "-").replace('/', "_")
}

#[cfg(test)]
mod fptplay_signing_tests {
    use super::*;

    #[test]
    fn token_uses_url_safe_unpadded_base64() {
        assert_eq!(
            fptplay_base64_urlsafe(&native_md5(b"abc")),
            "kAFQmDzST7DWlj99KOF_cg"
        );
    }
}
