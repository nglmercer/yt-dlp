const KANKANEWS_SIGN_SUFFIX: &str = "28c8edde3d61a0411511d3b1866f0636";

fn kankanews_nonce() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};

    const ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let time_seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let mut state = time_seed ^ COUNTER.fetch_add(1, Ordering::Relaxed).rotate_left(17);
    let mut nonce = String::with_capacity(8);
    for _ in 0..8 {
        // A nonce only needs to be unpredictable enough to avoid duplicate
        // request signatures; the API does not use it as a secret.
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        nonce.push(ALPHABET[(state % ALPHABET.len() as u64) as usize] as char);
    }
    nonce
}

fn kankanews_query(video_id: &str) -> Vec<(String, String)> {
    let fields = [
        ("nonce", kankanews_nonce()),
        ("omsid", video_id.to_owned()),
        ("platform", "pc".to_owned()),
        (
            "timestamp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs().to_string())
                .unwrap_or_else(|_| "0".to_owned()),
        ),
        ("version", "1.0".to_owned()),
    ];
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in &fields {
        serializer.append_pair(key, value);
    }
    let encoded = serializer.finish();
    let first = native_hex(
        &native_md5(format!("{encoded}&{KANKANEWS_SIGN_SUFFIX}").as_bytes()),
    );
    let sign = native_hex(&native_md5(first.as_bytes()));
    let mut query = fields
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect::<Vec<_>>();
    query.push(("sign".to_owned(), sign));
    query
}
