/// Native YouTube PO-token (proof-of-origin token) boundary.
/// Mirrors `yt_dlp/extractor/youtube/_base.py::_PoTokenContext`,
/// `yt_dlp/extractor/youtube/pot/provider.py`,
/// `yt_dlp/extractor/youtube/pot/utils.py`,
/// `yt_dlp/extractor/youtube/pot/cache.py` (key derivation only),
/// `yt_dlp/extractor/youtube/pot/_builtin/webpo_cachespec.py`,
/// `yt_dlp/extractor/youtube/pot/_builtin/memory_cache.py`, and the
/// `po_token`/`fetch_pot` configuration handling in
/// `yt_dlp/extractor/youtube/_video.py::YoutubeIE`.
///
/// Token *minting* (running WebPO/botguard challenges) stays an explicit
/// TODO: upstream delegates it to external `PoTokenProvider` plugins through
/// the director, and there is no native JS runtime for those challenges yet.
/// What this module does port is everything around that core that is
/// deterministic: contexts, request/response shapes, provider error kinds,
/// `CLIENT[.CONTEXT]+TOKEN` config parsing and validation, the
/// `fetch_pot` policy, WebPO content-binding rules, cache-spec generation,
/// cache-key derivation, the memory LRU cache, visitor/data-sync extraction,
/// and `serviceIntegrityDimensions` injection into player API payloads.

pub(crate) const YOUTUBE_POT_DEFAULT_TTL_SECS: u64 = 21600;
pub(crate) const YOUTUBE_POT_MEMORY_CACHE_SIZE: usize = 25;
pub(crate) const YOUTUBE_POT_CACHE_VERSION: &str = "v1";

/// Mirrors `PoTokenContext` (`GVS`/`PLAYER`/`SUBS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PoTokenContext {
    Gvs,
    Player,
    Subs,
}

impl PoTokenContext {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Gvs => "gvs",
            Self::Player => "player",
            Self::Subs => "subs",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "gvs" => Some(Self::Gvs),
            "player" => Some(Self::Player),
            "subs" => Some(Self::Subs),
            _ => None,
        }
    }
}

/// Mirrors the `fetch_pot` extractor-arg policy in `_fetch_po_token`.
/// Unknown values fall back to `Auto`, matching the Python default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FetchPotPolicy {
    Never,
    Auto,
    Always,
}

impl FetchPotPolicy {
    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "never" => Self::Never,
            "always" => Self::Always,
            _ => Self::Auto,
        }
    }

    /// Mirrors the early return in `_fetch_po_token`: `never` always skips,
    /// and `auto` only fetches when the token is required.
    pub(crate) fn should_fetch(self, required: bool) -> bool {
        match self {
            Self::Never => false,
            Self::Always => true,
            Self::Auto => required,
        }
    }
}

/// Mirrors `IEContentProviderError`/`PoTokenProviderError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PoTokenProviderError {
    pub(crate) message: String,
    pub(crate) expected: bool,
}

impl PoTokenProviderError {
    pub(crate) fn new(message: impl Into<String>, expected: bool) -> Self {
        Self {
            message: message.into(),
            expected,
        }
    }
}

/// Mirrors `PoTokenProviderRejectedRequest`: the provider cannot handle the
/// request, so the director must try the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PoTokenRequestRejected;

/// Mirrors `PoTokenResponse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PoTokenResponse {
    pub(crate) po_token: String,
    pub(crate) expires_at: Option<u64>,
}

/// Mirrors the deterministic fields of `PoTokenRequest`. Networking handles
/// (cookie jar, proxy, headers) stay with the caller: the native request
/// director owns them and providers receive plain values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PoTokenRequest {
    pub(crate) context: PoTokenContext,
    pub(crate) client_name: String,
    pub(crate) visitor_data: Option<String>,
    pub(crate) data_sync_id: Option<String>,
    pub(crate) video_id: Option<String>,
    pub(crate) session_index: Option<String>,
    pub(crate) player_url: Option<String>,
    pub(crate) is_authenticated: bool,
    pub(crate) gvs_bind_to_video_id: bool,
    pub(crate) bypass_cache: bool,
}

/// Outcome of parsing the `po_token` extractor-arg list, mirroring
/// `_get_config_po_token`. Entries that name a different client or context
/// are skipped silently; malformed entries are skipped with a warning and
/// parsing continues with the next entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigPoTokenOutcome {
    pub(crate) token: Option<String>,
    pub(crate) warnings: Vec<String>,
}

/// Mirrors `_get_config_po_token`: entries look like `CLIENT[.CONTEXT]+TOKEN`
/// (casesense values, case-insensitive client/context match). A missing
/// context defaults to GVS. The token is percent-decoded, base64url-decoded,
/// and re-encoded to canonical padded base64url, which also strips stray
/// characters such as accidentally pasted URL params.
pub(crate) fn parse_config_po_token(
    entries: &[&str],
    client: &str,
    context: PoTokenContext,
) -> ConfigPoTokenOutcome {
    let mut outcome = ConfigPoTokenOutcome {
        token: None,
        warnings: Vec::new(),
    };
    for entry in entries {
        let Some((meta, token)) = entry.split_once('+') else {
            outcome.warnings.push(format!(
                "Invalid po_token configuration format. Expected \"CLIENT.CONTEXT+PO_TOKEN\", got \"{entry}\""
            ));
            continue;
        };
        let (entry_client, entry_context) = match meta.split_once('.') {
            Some((entry_client, entry_context)) => (entry_client, entry_context.to_owned()),
            // Mirrors the Python branch: a bare `CLIENT+TOKEN` assumes GVS.
            None => (meta, PoTokenContext::Gvs.as_str().to_owned()),
        };
        if !entry_client.eq_ignore_ascii_case(client) {
            continue;
        }
        if !entry_context.eq_ignore_ascii_case(context.as_str()) {
            continue;
        }
        match canonicalize_po_token(token) {
            Some(canonical) => {
                outcome.token = Some(canonical);
                return outcome;
            }
            None => outcome.warnings.push(format!(
                "Invalid po_token configuration for {client} client: \
                 {entry_context} PO Token should be a base64url-encoded string."
            )),
        }
    }
    outcome
}

fn canonicalize_po_token(token: &str) -> Option<String> {
    let decoded = pot_base64url_decode(&pot_percent_decode(token))?;
    Some(pot_base64url_encode(&decoded))
}

fn pot_percent_decode(value: &str) -> Vec<u8> {
    fn hex_digit(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
            {
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    decoded
}

/// Base64url decode mirroring `base64.urlsafe_b64decode` on top of
/// `urllib.parse.unquote`, as used by `_get_config_po_token` and
/// `_extract_visitor_id`. The rules below were pinned against CPython
/// behavior probe by probe: `-`/`_` translate to `+/` (and pre-existing
/// `+/` still decode), every other non-alphabet byte is discarded, and only
/// the data length plus trailing padding gate success. A data length one
/// past a multiple of four always fails; a short final quantum needs
/// trailing `=` cover (two pads for two digits, one pad for three digits).
/// Non-ASCII input fails, mirroring the stdlib `ValueError`.
fn pot_base64url_decode(value: &[u8]) -> Option<Vec<u8>> {
    if value.iter().any(|byte| !byte.is_ascii()) {
        return None;
    }
    let mut filtered: Vec<u8> = Vec::with_capacity(value.len());
    for byte in value.iter().copied() {
        if matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'-' | b'_' | b'='
        ) {
            filtered.push(byte);
        }
    }
    let trailing_pads = filtered
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .count();
    let mut digits: Vec<u8> = Vec::with_capacity(filtered.len());
    for byte in filtered.iter().copied().filter(|byte| *byte != b'=') {
        digits.push(match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            _ => 63, // b'/' | b'_'
        });
    }
    match digits.len() % 4 {
        1 => return None,
        2 if trailing_pads < 2 => return None,
        3 if trailing_pads < 1 => return None,
        _ => {}
    }
    let mut output = Vec::with_capacity(digits.len() / 4 * 3 + 2);
    for chunk in digits.chunks(4) {
        let mut quantum = (u32::from(chunk[0]) << 18) | (u32::from(chunk[1]) << 12);
        output.push((quantum >> 16) as u8);
        if chunk.len() > 2 {
            quantum |= u32::from(chunk[2]) << 6;
            output.push((quantum >> 8) as u8);
        }
        if chunk.len() > 3 {
            quantum |= u32::from(chunk[3]);
            output.push(quantum as u8);
        }
    }
    Some(output)
}

/// Canonical padded base64url encode, mirroring `base64.urlsafe_b64encode`.
fn pot_base64url_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut encoded = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(ALPHABET[(first >> 2) as usize] as char);
        encoded.push(ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(ALPHABET[(third & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

/// Mirrors `WEBPO_CLIENTS` in `pot/utils.py`.
pub(crate) const YOUTUBE_WEBPO_CLIENTS: &[&str] = &[
    "WEB",
    "MWEB",
    "TVHTML5",
    "WEB_EMBEDDED_PLAYER",
    "WEB_CREATOR",
    "WEB_REMIX",
    "TVHTML5_SIMPLY",
    "TVHTML5_SIMPLY_EMBEDDED_PLAYER",
];

/// Mirrors `ContentBindingType` in `pot/utils.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PoTokenContentBindingType {
    VisitorData,
    DatasyncId,
    VideoId,
    VisitorId,
}

impl PoTokenContentBindingType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::VisitorData => "visitor_data",
            Self::DatasyncId => "datasync_id",
            Self::VideoId => "video_id",
            Self::VisitorId => "visitor_id",
        }
    }
}

/// Mirrors `pot/utils.py::_extract_visitor_id`: the visitor ID is bytes 2..13
/// of the base64url-decoded visitor data (after `unquote_plus`), and must be
/// 11 word characters. Ideally this would use a protobuf parser; the slice
/// matches the Python heuristic exactly.
pub(crate) fn extract_visitor_id(visitor_data: &str) -> Option<String> {
    let plus_decoded: Vec<u8> = visitor_data
        .bytes()
        .flat_map(|byte| if byte == b'+' { vec![b' '] } else { vec![byte] })
        .collect();
    let decoded = pot_base64url_decode(&pot_percent_decode_bytes(&plus_decoded))?;
    let id_bytes = decoded.get(2..13)?;
    let id = std::str::from_utf8(id_bytes).ok()?;
    if id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        Some(id.to_owned())
    } else {
        None
    }
}

fn pot_percent_decode_bytes(value: &[u8]) -> Vec<u8> {
    pot_percent_decode(std::str::from_utf8(value).unwrap_or(""))
}

/// Mirrors `pot/utils.py::get_webpo_content_binding`.
pub(crate) fn webpo_content_binding(
    client_name: Option<&str>,
    context: PoTokenContext,
    is_authenticated: bool,
    visitor_data: Option<&str>,
    data_sync_id: Option<&str>,
    video_id: Option<&str>,
    gvs_bind_to_video_id: bool,
    bind_to_visitor_id: bool,
) -> Option<(String, PoTokenContentBindingType)> {
    let client_name = client_name?;
    if !YOUTUBE_WEBPO_CLIENTS.contains(&client_name) {
        return None;
    }
    if context == PoTokenContext::Gvs && gvs_bind_to_video_id {
        return video_id
            .filter(|id| !id.is_empty())
            .map(|id| (id.to_owned(), PoTokenContentBindingType::VideoId));
    }
    if context == PoTokenContext::Gvs || client_name == "WEB_REMIX" {
        if is_authenticated {
            return data_sync_id
                .filter(|id| !id.is_empty())
                .map(|id| (id.to_owned(), PoTokenContentBindingType::DatasyncId));
        }
        if bind_to_visitor_id {
            if let Some(visitor_id) = visitor_data.and_then(extract_visitor_id) {
                return Some((visitor_id, PoTokenContentBindingType::VisitorId));
            }
        }
        return visitor_data
            .filter(|data| !data.is_empty())
            .map(|data| (data.to_owned(), PoTokenContentBindingType::VisitorData));
    }
    if matches!(context, PoTokenContext::Player | PoTokenContext::Subs) {
        return video_id
            .filter(|id| !id.is_empty())
            .map(|id| (id.to_owned(), PoTokenContentBindingType::VideoId));
    }
    None
}

/// Mirrors `CacheProviderWritePolicy` in `pot/cache.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PoTokenCacheWritePolicy {
    WriteAll,
    WriteFirst,
}

/// Mirrors `PoTokenCacheSpec` for the `webpo` spec provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebPoCacheSpec {
    /// `(key, value)` bindings; `None` values are dropped when keying,
    /// mirroring `_generate_key_bindings`.
    pub(crate) bindings: Vec<(String, Option<String>)>,
    pub(crate) default_ttl_secs: u64,
    pub(crate) write_policy: PoTokenCacheWritePolicy,
}

/// Mirrors `WebPoPCSP::generate_cache_spec`, including the 6-hour default TTL
/// (half the 12-hour integrity-token TTL, to be safe) and the `WRITE_FIRST`
/// policy for video-ID bindings.
pub(crate) fn webpo_cache_spec(
    request: &PoTokenRequest,
    innertube_remote_host: Option<&str>,
    request_source_address: Option<&str>,
    request_proxy: Option<&str>,
    bind_to_visitor_id: bool,
) -> Option<WebPoCacheSpec> {
    let (binding, binding_type) = webpo_content_binding(
        Some(request.client_name.as_str()),
        request.context,
        request.is_authenticated,
        request.visitor_data.as_deref(),
        request.data_sync_id.as_deref(),
        request.video_id.as_deref(),
        request.gvs_bind_to_video_id,
        bind_to_visitor_id,
    )?;
    let write_policy = if binding_type == PoTokenContentBindingType::VideoId {
        PoTokenCacheWritePolicy::WriteFirst
    } else {
        PoTokenCacheWritePolicy::WriteAll
    };
    Some(WebPoCacheSpec {
        bindings: vec![
            ("t".to_owned(), Some("webpo".to_owned())),
            ("cb".to_owned(), Some(binding)),
            ("cbt".to_owned(), Some(binding_type.as_str().to_owned())),
            ("ip".to_owned(), innertube_remote_host.map(str::to_owned)),
            ("sa".to_owned(), request_source_address.map(str::to_owned)),
            ("px".to_owned(), request_proxy.map(str::to_owned)),
        ],
        default_ttl_secs: YOUTUBE_POT_DEFAULT_TTL_SECS,
        write_policy,
    })
}

/// Mirrors `PoTokenCache::_generate_key_bindings` plus `_generate_key`: drop
/// `None` bindings, pin `_dlp_cache` to the cache version and `_p` to the
/// spec-provider key, then SHA-256 the Python `repr` of the sorted mapping.
pub(crate) fn pot_cache_key(provider_key: &str, bindings: &[(String, Option<String>)]) -> String {
    let mut cleaned: Vec<(&str, &str)> = bindings
        .iter()
        .filter_map(|(key, value)| value.as_deref().map(|value| (key.as_str(), value)))
        .collect();
    cleaned.push(("_dlp_cache", YOUTUBE_POT_CACHE_VERSION));
    cleaned.push(("_p", provider_key));
    cleaned.sort_unstable();
    let repr = python_str_dict_repr(&cleaned);
    let digest = pot_sha256(repr.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn python_str_dict_repr(entries: &[(&str, &str)]) -> String {
    let mut repr = String::from("{");
    for (index, (key, value)) in entries.iter().enumerate() {
        if index > 0 {
            repr.push_str(", ");
        }
        repr.push_str(&python_str_repr(key));
        repr.push_str(": ");
        repr.push_str(&python_str_repr(value));
    }
    repr.push('}');
    repr
}

/// Single-quoted Python string repr for the ASCII binding values cache keys
/// carry (tokens, IDs, hosts, proxies); mirrors CPython's quoting choice for
/// strings without single quotes or newlines.
fn python_str_repr(value: &str) -> String {
    let mut repr = String::with_capacity(value.len() + 2);
    repr.push('\'');
    for byte in value.bytes() {
        match byte {
            b'\'' => repr.push_str("\\'"),
            b'\\' => repr.push_str("\\\\"),
            b'\n' => repr.push_str("\\n"),
            b'\r' => repr.push_str("\\r"),
            b'\t' => repr.push_str("\\t"),
            0x20..=0x7e => repr.push(byte as char),
            _ => repr.push(byte as char),
        }
    }
    repr.push('\'');
    repr
}

fn pot_sha256(message: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (message.len() as u64).wrapping_mul(8);
    let mut padded = message.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for block in padded.chunks_exact(64) {
        let mut schedule = [0u32; 64];
        for (index, word) in schedule.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                block[offset],
                block[offset + 1],
                block[offset + 2],
                block[offset + 3],
            ]);
        }
        for index in 16..64 {
            let small_sigma_0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let small_sigma_1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(small_sigma_0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(small_sigma_1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) = (
            state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7],
        );
        for index in 0..64 {
            let big_sigma_1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp_1 = h
                .wrapping_add(big_sigma_1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let big_sigma_0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp_2 = big_sigma_0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp_1);
            d = c;
            c = b;
            b = a;
            a = temp_1.wrapping_add(temp_2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }
    let mut digest = [0u8; 32];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

/// Mirrors `MemoryLRUPCP`: an insertion-ordered LRU map of cache key to
/// `(token, expires_at)` with second-granularity expiry. `now_secs` is a
/// parameter so tests stay deterministic; callers pass the current UTC epoch.
#[derive(Debug, Clone, Default)]
pub(crate) struct MemoryPoTokenCache {
    entries: Vec<(String, String, u64)>,
    max_size: usize,
}

impl MemoryPoTokenCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_size: YOUTUBE_POT_MEMORY_CACHE_SIZE,
        }
    }

    pub(crate) fn get(&mut self, key: &str, now_secs: u64) -> Option<String> {
        let position = self.entries.iter().position(|(entry, _, _)| entry == key)?;
        let (_, token, expires_at) = self.entries.remove(position);
        if expires_at < now_secs {
            return None;
        }
        self.entries
            .push((key.to_owned(), token.clone(), expires_at));
        Some(token)
    }

    pub(crate) fn store(&mut self, key: &str, token: &str, expires_at: u64, now_secs: u64) {
        if expires_at < now_secs {
            return;
        }
        if let Some(position) = self.entries.iter().position(|(entry, _, _)| entry == key) {
            self.entries.remove(position);
        }
        self.entries
            .push((key.to_owned(), token.to_owned(), expires_at));
        if self.entries.len() > self.max_size {
            self.entries.remove(0);
        }
    }

    pub(crate) fn delete(&mut self, key: &str) {
        if let Some(position) = self.entries.iter().position(|(entry, _, _)| entry == key) {
            self.entries.remove(position);
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Mirrors `YoutubeIE._extract_visitor_data` (response/ytcfg half; the
/// `visitor_data` extractor-arg override is applied by the caller that owns
/// argument plumbing): first string found at `VISITOR_DATA`,
/// `INNERTUBE_CONTEXT.client.visitorData`, or `responseContext.visitorData`.
pub(crate) fn youtube_visitor_data(candidates: &[serde_json::Value]) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        youtube_json_string(candidate, "VISITOR_DATA")
            .or_else(|| {
                candidate
                    .get("INNERTUBE_CONTEXT")?
                    .get("client")?
                    .get("visitorData")?
                    .as_str()
                    .map(str::to_owned)
            })
            .or_else(|| {
                candidate
                    .get("responseContext")?
                    .get("visitorData")?
                    .as_str()
                    .map(str::to_owned)
            })
    })
}

/// Mirrors `YoutubeIE._extract_data_sync_id` (response/ytcfg half):
/// `DATASYNC_ID` or `responseContext.mainAppWebResponseContext.datasyncId`.
pub(crate) fn youtube_data_sync_id(candidates: &[serde_json::Value]) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        youtube_json_string(candidate, "DATASYNC_ID").or_else(|| {
            candidate
                .get("responseContext")?
                .get("mainAppWebResponseContext")?
                .get("datasyncId")?
                .as_str()
                .map(str::to_owned)
        })
    })
}

/// Player PO Token from the `po_token` extractor-arg, mirroring the config
/// branch of `fetch_po_token` for the `web` client and PLAYER context. Empty
/// tokens are dropped, mirroring the truthiness check at the Python call
/// site. Director fetching stays TODO, so only configured tokens flow today.
pub(crate) fn youtube_configured_player_po_token(context: &ExtractionContext) -> Option<String> {
    let entries = context.configuration_arg("Youtube", "po_token", true);
    let refs: Vec<&str> = entries.iter().map(String::as_str).collect();
    parse_config_po_token(&refs, "web", PoTokenContext::Player)
        .token
        .filter(|token| !token.is_empty())
}

/// Visitor data with the `visitor_data` extractor-arg override first,
/// mirroring `_extract_visitor_data`: an explicit configured value wins,
/// otherwise the first page/ytcfg candidate wins.
pub(crate) fn youtube_configured_visitor_data(
    context: &ExtractionContext,
    candidates: &[serde_json::Value],
) -> Option<String> {
    context
        .configuration_arg("Youtube", "visitor_data", true)
        .into_iter()
        .next()
        .filter(|value| !value.is_empty())
        .or_else(|| youtube_visitor_data(candidates))
}

/// Injects `serviceIntegrityDimensions: {poToken}` into a player API payload,
/// mirroring `_extract_player_response`.
pub(crate) fn apply_player_po_token(payload: &mut serde_json::Value, po_token: &str) {
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "serviceIntegrityDimensions".to_owned(),
            serde_json::json!({ "poToken": po_token }),
        );
    }
}
