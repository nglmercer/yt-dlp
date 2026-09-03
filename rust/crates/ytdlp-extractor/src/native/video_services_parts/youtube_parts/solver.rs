/// Native YouTube `signatureCipher`/`n` challenge solving through the EJS
/// challenge protocol (mirrors `yt_dlp/extractor/youtube/jsc/`).
///
/// The vendored solver scripts below are the exact pinned EJS release
/// (`vendor/_info.py`): they are embedded with `include_str!` and verified
/// against the pinned SHA3-512 hashes before use, mirroring
/// `_ALLOWED_HASHES`. Challenge *requests* follow `_construct_stdin`
/// (`{type, challenges}` per type, player script inline); *responses* follow
/// `_real_bulk_solve` (`{responses: [{data}]}` zipped with the requests, or
/// `{type: 'error'}`). Solved URLs are rewritten exactly like
/// `process_https_formats`/`process_manifest_format`; anything unsolved keeps
/// its explicit TODO. Without a usable JavaScript runtime or solver script,
/// extraction continues with TODOs instead of failing.

const YOUTUBE_SOLVER_VERSION: &str = "0.8.0";
const YOUTUBE_SOLVER_CORE_JS: &str = include_str!(
    "../../../../../../../yt_dlp/extractor/youtube/jsc/_builtin/vendor/yt.solver.core.js"
);
const YOUTUBE_SOLVER_DENO_LIB_JS: &str = include_str!(
    "../../../../../../../yt_dlp/extractor/youtube/jsc/_builtin/vendor/yt.solver.deno.lib.js"
);
const YOUTUBE_SOLVER_BUN_LIB_JS: &str = include_str!(
    "../../../../../../../yt_dlp/extractor/youtube/jsc/_builtin/vendor/yt.solver.bun.lib.js"
);

/// Pinned script hashes, ported 1:1 from `vendor/_info.py` (`HASHES`).
fn youtube_solver_pinned_hashes() -> [(&'static str, &'static str, &'static str); 3] {
    [
        (
            "yt.solver.core.js",
            YOUTUBE_SOLVER_CORE_JS,
            "c163a6f376db6ce3da47d516a28a8f2a0554ae95c58dc766f0a6e2b3894f2cef1ee07fa84beb442fa471aac4f300985added1657c7c94c4d1cfefe68920ab599",
        ),
        (
            "yt.solver.deno.lib.js",
            YOUTUBE_SOLVER_DENO_LIB_JS,
            "9c8ee3ab6c23e443a5a951e3ac73c6b8c1c8fb34335e7058a07bf99d349be5573611de00536dcd03ecd3cf34014c4e9b536081de37af3637c5390c6a6fd6a0f0",
        ),
        (
            "yt.solver.bun.lib.js",
            YOUTUBE_SOLVER_BUN_LIB_JS,
            "6ff45e94de9f0ea936a183c48173cfa9ce526ee4b7544cd556428427c1dd53c8073ef0174e79b320252bf0e7c64b0032cc1cf9c4358f3fda59033b7caa01c241",
        ),
    ]
}

pub(crate) fn youtube_verify_solver_scripts() -> Result<(), String> {
    let mut mismatches = Vec::new();
    for (filename, code, expected) in youtube_solver_pinned_hashes() {
        let actual = sha3_512_hex(code.as_bytes());
        if actual != expected {
            mismatches.push(format!("{filename} (solver v{YOUTUBE_SOLVER_VERSION})"));
        }
    }
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "challenge solver script hash mismatch: {}",
            mismatches.join(", ")
        ))
    }
}

// --- Minimal SHA3-512 (FIPS 202), dependency-free. ---

const KECCAK_ROUND_CONSTANTS: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

const KECCAK_ROTATION_OFFSETS: [[u32; 5]; 5] = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14],
];

fn keccak_f1600(state: &mut [u64; 25]) {
    let mut lanes = [0u64; 25];
    for round in 0..24 {
        // Theta.
        let mut parity = [0u64; 5];
        for x in 0..5 {
            parity[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        for x in 0..5 {
            let mixed = parity[(x + 4) % 5] ^ parity[(x + 1) % 5].rotate_left(1);
            for y in 0..5 {
                state[x + 5 * y] ^= mixed;
            }
        }
        // Rho and pi.
        for x in 0..5 {
            for y in 0..5 {
                let source = x + 5 * y;
                let target = y + 5 * ((2 * x + 3 * y) % 5);
                lanes[target] = state[source].rotate_left(KECCAK_ROTATION_OFFSETS[x][y]);
            }
        }
        // Chi.
        for y in 0..5 {
            for x in 0..5 {
                state[x + 5 * y] =
                    lanes[x + 5 * y] ^ ((!lanes[(x + 1) % 5 + 5 * y]) & lanes[(x + 2) % 5 + 5 * y]);
            }
        }
        // Iota.
        state[0] ^= KECCAK_ROUND_CONSTANTS[round];
    }
}

pub(crate) fn sha3_512_hex(data: &[u8]) -> String {
    const RATE: usize = 72;
    let mut state = [0u64; 25];
    let mut blocks = data.chunks_exact(RATE);
    for block in &mut blocks {
        for (index, lane) in block.chunks_exact(8).enumerate() {
            state[index] ^= u64::from_le_bytes(lane.try_into().unwrap_or([0; 8]));
        }
        keccak_f1600(&mut state);
    }
    let mut last = [0u8; RATE];
    let remainder = blocks.remainder();
    last[..remainder.len()].copy_from_slice(remainder);
    last[remainder.len()] ^= 0x06;
    last[RATE - 1] ^= 0x80;
    for (index, lane) in last.chunks_exact(8).enumerate() {
        state[index] ^= u64::from_le_bytes(lane.try_into().unwrap_or([0; 8]));
    }
    keccak_f1600(&mut state);
    let mut hex = String::with_capacity(128);
    for lane in state.iter().take(8) {
        for byte in lane.to_le_bytes() {
            hex.push_str(&format!("{byte:02x}"));
        }
    }
    hex
}

// --- Challenge model (mirrors `jsc/provider.py`). ---

pub(crate) struct YoutubeSigChallenge {
    pub format_index: usize,
    /// The encrypted signature bytes from the `s` cipher parameter.
    pub encrypted: String,
    /// The signature query parameter name (`sp`, default `signature`).
    pub param: String,
}

pub(crate) struct YoutubeNChallenge {
    pub format_index: usize,
    pub value: String,
    /// Whether the challenge lives in an `/n/<value>/` path segment
    /// (manifest URLs) rather than the `n` query parameter.
    pub in_path: bool,
}

#[derive(Default)]
pub(crate) struct YoutubeChallenges {
    pub sig: Vec<YoutubeSigChallenge>,
    pub n: Vec<YoutubeNChallenge>,
}

impl YoutubeChallenges {
    pub(crate) fn is_empty(&self) -> bool {
        self.sig.is_empty() && self.n.is_empty()
    }

    /// Distinct signature lengths, mirroring the `s_challenges` set of
    /// encrypted-signature lengths in `_extract_formats_and_subtitles`.
    pub(crate) fn sig_lengths(&self) -> Vec<usize> {
        let mut lengths = self
            .sig
            .iter()
            .map(|challenge| challenge.encrypted.chars().count())
            .collect::<Vec<_>>();
        lengths.sort_unstable();
        lengths.dedup();
        lengths
    }

    pub(crate) fn n_values(&self) -> Vec<String> {
        let mut values = self
            .n
            .iter()
            .map(|challenge| challenge.value.clone())
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        values
    }
}

/// Dummy SIG challenge strings, mirroring
/// `''.join(map(chr, range(spec_id)))`: the solver only needs the length to
/// identify the decipher function, and the response is keyed by the dummy.
pub(crate) fn youtube_sig_dummy(spec_id: usize) -> String {
    (0..spec_id as u32).filter_map(char::from_u32).collect()
}

/// Mirror of the local `solve_sig`: index the encrypted signature by the
/// solver-returned ordinal string.
pub(crate) fn youtube_apply_sig_spec(encrypted: &str, spec: &str) -> Option<String> {
    let chars = encrypted.chars().collect::<Vec<_>>();
    spec.chars()
        .map(|index| chars.get(index as usize).copied())
        .collect::<Option<String>>()
}

// --- EJS stdin/stdout protocol (mirrors `_construct_stdin`). ---

pub(crate) fn youtube_solver_stdin(
    player_js: &str,
    library_js: &str,
    core_js: &str,
    sig_dummies: &[String],
    n_values: &[String],
) -> String {
    let mut requests = Vec::new();
    if !n_values.is_empty() {
        requests.push(serde_json::json!({"type": "n", "challenges": n_values}));
    }
    if !sig_dummies.is_empty() {
        requests.push(serde_json::json!({"type": "sig", "challenges": sig_dummies}));
    }
    let input = serde_json::json!({
        "type": "player",
        "player": player_js,
        "requests": requests,
        "output_preprocessed": true,
    });
    yt_dlp_javascript::build_ejs_script(library_js, core_js, &input)
}

#[derive(Default)]
pub(crate) struct YoutubeSolutions {
    /// Decipher ordinal strings keyed by encrypted-signature length.
    pub sig_specs: std::collections::BTreeMap<usize, String>,
    /// Solved throttling parameters keyed by challenge value.
    pub n_results: std::collections::BTreeMap<String, String>,
    /// Per-request failures that left some challenges unsolved. Callers keep
    /// the corresponding TODOs instead of failing the whole extraction.
    pub errors: Vec<String>,
}

/// Parse a solver response, zipping `{responses: [{data}]}` with the request
/// order exactly like `_real_bulk_solve`.
pub(crate) fn youtube_parse_solver_output(
    stdout: &str,
    sig_dummies: &[String],
    n_values: &[String],
) -> Result<YoutubeSolutions, String> {
    let output = yt_dlp_javascript::parse_json_output(stdout)
        .map_err(|error| format!("invalid solver output: {error}"))?;
    if output.get("type").and_then(serde_json::Value::as_str) == Some("error") {
        return Err(output
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown solver error")
            .to_owned());
    }
    let responses = output
        .get("responses")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "solver output has no responses array".to_owned())?;
    let mut ordered = Vec::new();
    if !n_values.is_empty() {
        ordered.push("n");
    }
    if !sig_dummies.is_empty() {
        ordered.push("sig");
    }
    if responses.len() != ordered.len() {
        return Err(format!(
            "solver returned {} responses for {} requests",
            responses.len(),
            ordered.len()
        ));
    }
    let mut solutions = YoutubeSolutions::default();
    for (kind, response) in ordered.into_iter().zip(responses.iter()) {
        if response.get("type").and_then(serde_json::Value::as_str) == Some("error") {
            let detail = response
                .get("error")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| response.to_string());
            solutions
                .errors
                .push(format!("solver {kind} challenge failed: {detail}"));
            continue;
        }
        let Some(data) = response.get("data").and_then(serde_json::Value::as_object) else {
            solutions
                .errors
                .push(format!("solver {kind} response has no result data object"));
            continue;
        };
        if kind == "n" {
            for value in n_values {
                if let Some(result) = data.get(value).and_then(serde_json::Value::as_str) {
                    solutions.n_results.insert(value.clone(), result.to_owned());
                }
            }
        } else {
            for dummy in sig_dummies {
                if let Some(result) = data.get(dummy).and_then(serde_json::Value::as_str) {
                    solutions
                        .sig_specs
                        .insert(dummy.chars().count(), result.to_owned());
                }
            }
        }
    }
    Ok(solutions)
}

// --- Runtime selection (mirrors provider preferences). ---

/// Library shim per runtime. Node and QuickJS have no vendored lib script,
/// mirroring the builtin source fallback chain (pypackage/cache/web only).
pub(crate) fn youtube_solver_library(
    kind: yt_dlp_javascript::RuntimeKind,
) -> Option<&'static str> {
    match kind {
        yt_dlp_javascript::RuntimeKind::Deno => Some(YOUTUBE_SOLVER_DENO_LIB_JS),
        yt_dlp_javascript::RuntimeKind::Bun => Some(YOUTUBE_SOLVER_BUN_LIB_JS),
        yt_dlp_javascript::RuntimeKind::Node | yt_dlp_javascript::RuntimeKind::QuickJs => None,
    }
}

fn youtube_strip_terminal_sequences(line: &str) -> String {
    Regex::new("\x1b\\[[^m]+m")
        .ok()
        .map(|matcher| matcher.replace_all(line, "").into_owned())
        .unwrap_or_else(|| line.to_owned())
}

/// Mirror the provider `_clean_stderr` filters: benign runtime banners are
/// ignored, while any remaining stderr fails the solver invocation.
pub(crate) fn youtube_clean_solver_stderr(
    kind: yt_dlp_javascript::RuntimeKind,
    stderr: &str,
) -> String {
    stderr
        .lines()
        .filter(|line| {
            let line = youtube_strip_terminal_sequences(line);
            !youtube_solver_stderr_ignored(kind, &line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn youtube_solver_stderr_ignored(kind: yt_dlp_javascript::RuntimeKind, line: &str) -> bool {
    use yt_dlp_javascript::RuntimeKind;
    match kind {
        RuntimeKind::Deno => {
            Regex::new(r"^Download https\S+$")
                .ok()
                .is_some_and(|matcher| matcher.is_match(line).unwrap_or(false))
                || line
                    .starts_with("DANGER: TLS certificate validation is disabled for all hostnames")
        }
        RuntimeKind::Node => {
            line.starts_with("[stdin]:")
                || line.starts_with("var jsc")
                || line
                    == "(Use `node --trace-uncaught ...` to show where the exception was thrown)"
                || Regex::new(r"^Node\.js v\d+\.\d+\.\d+$")
                    .ok()
                    .is_some_and(|matcher| matcher.is_match(line).unwrap_or(false))
        }
        RuntimeKind::Bun => Regex::new(r"^Bun v\d+\.\d+\.\d+ \([\w\s]+\)$")
            .ok()
            .is_some_and(|matcher| matcher.is_match(line).unwrap_or(false)),
        RuntimeKind::QuickJs => false,
    }
}

/// Select the first usable runtime from provider preference order
/// (deno 1000, node 900, quickjs 850, bun 800). A runtime without a solver
/// library, an unavailable executable, a probe error, or an unsupported
/// version is skipped in favour of the next provider.
pub(crate) fn youtube_select_solver_runtime_with(
    probe: impl Fn(
        yt_dlp_javascript::RuntimeKind,
    ) -> Option<(yt_dlp_javascript::JavascriptRuntime, &'static str)>,
) -> Option<(yt_dlp_javascript::JavascriptRuntime, &'static str)> {
    use yt_dlp_javascript::RuntimeKind;
    for kind in [
        RuntimeKind::Deno,
        RuntimeKind::Node,
        RuntimeKind::QuickJs,
        RuntimeKind::Bun,
    ] {
        if let Some(selected) = probe(kind) {
            return Some(selected);
        }
    }
    None
}

/// Probe runtimes in provider preference order and return the first usable
/// one with its library shim.
pub(crate) fn youtube_select_solver_runtime(
) -> Option<(yt_dlp_javascript::JavascriptRuntime, &'static str)> {
    use yt_dlp_javascript::RuntimeKind;
    youtube_select_solver_runtime_with(|kind| {
        let library = youtube_solver_library(kind)?;
        let runtime = yt_dlp_javascript::JavascriptRuntime::probe(kind, None)
            .ok()
            .flatten()?;
        runtime.info().supported.then_some((runtime, library))
    })
}

pub(crate) fn youtube_node_extra_args(runtime: &yt_dlp_javascript::JavascriptRuntime) -> Vec<String> {
    if runtime.info().kind != yt_dlp_javascript::RuntimeKind::Node {
        return Vec::new();
    }
    // Mirrors the permission flags in the Node provider.
    if runtime.info().version_tuple.as_slice() >= [23, 5, 0].as_slice() {
        vec!["--permission".to_owned()]
    } else {
        vec![
            "--experimental-permission".to_owned(),
            "--no-warnings=ExperimentalWarning".to_owned(),
        ]
    }
}

/// Bulk-solve signature and `n` challenges against one player script.
/// Failures (no runtime, missing scripts, solver errors) return `Err` and
/// callers keep the explicit TODOs; extraction never fails here.
pub(crate) fn youtube_bulk_solve(
    player_js: &str,
    challenges: &YoutubeChallenges,
) -> Result<YoutubeSolutions, ExtractorError> {
    youtube_verify_solver_scripts().map_err(|message| {
        ExtractorError::new(ExtractorErrorKind::Unsupported, format!("TODO: {message}"))
    })?;
    let (runtime, library) = youtube_select_solver_runtime().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            "TODO: YouTube challenge solving needs a supported JavaScript runtime with solver scripts (checked Deno, Node, QuickJS, Bun)",
        )
    })?;
    youtube_bulk_solve_with(
        player_js,
        challenges,
        &runtime,
        library,
        YOUTUBE_SOLVER_CORE_JS,
    )
}

pub(crate) fn youtube_bulk_solve_with(
    player_js: &str,
    challenges: &YoutubeChallenges,
    runtime: &yt_dlp_javascript::JavascriptRuntime,
    library_js: &str,
    core_js: &str,
) -> Result<YoutubeSolutions, ExtractorError> {
    let sig_dummies = challenges
        .sig_lengths()
        .into_iter()
        .map(youtube_sig_dummy)
        .collect::<Vec<_>>();
    let n_values = challenges.n_values();
    let stdin = youtube_solver_stdin(player_js, library_js, core_js, &sig_dummies, &n_values);
    let mut options = yt_dlp_javascript::RuntimeOptions::default();
    options.extra_args = youtube_node_extra_args(runtime);
    let output = runtime.execute(&stdin, &options).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: YouTube challenge solver failed: {error}"),
        )
    })?;
    let stderr = youtube_clean_solver_stderr(runtime.info().kind, &output.stderr);
    if !stderr.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: YouTube challenge solver runtime reported an error: {stderr}"),
        ));
    }
    let stdout = output.stdout.clone();
    youtube_parse_solver_output(&stdout, &sig_dummies, &n_values).map_err(|message| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: YouTube challenge solver returned unusable output: {message}"),
        )
    })
}

/// Rewrite solved format URLs in place. Returns whether every signature and
/// every `n` challenge was solved, so callers can prune TODO groups.
pub(crate) fn youtube_apply_solutions(
    formats: &mut [serde_json::Value],
    challenges: &YoutubeChallenges,
    solutions: &YoutubeSolutions,
) -> (bool, bool) {
    let mut sig_solved = true;
    for challenge in &challenges.sig {
        let solved = solutions
            .sig_specs
            .get(&challenge.encrypted.chars().count())
            .and_then(|spec| youtube_apply_sig_spec(&challenge.encrypted, spec))
            .and_then(|deciphered| {
                let format = formats.get(challenge.format_index)?;
                let url = format.get("url").and_then(serde_json::Value::as_str)?;
                youtube_replace_query(url, &challenge.param, &deciphered)
            });
        if let (Some(format), Some(url)) = (formats.get_mut(challenge.format_index), solved) {
            if let Some(object) = format.as_object_mut() {
                object.insert("url".to_owned(), serde_json::json!(url));
                object.remove("rust_todo");
            }
        } else {
            sig_solved = false;
        }
    }
    let mut n_solved = true;
    for challenge in &challenges.n {
        let solved = solutions
            .n_results
            .get(&challenge.value)
            .and_then(|result| {
                let format = formats.get(challenge.format_index)?;
                let url = format.get("url").and_then(serde_json::Value::as_str)?;
                if challenge.in_path {
                    youtube_replace_n_path_segment(url, &challenge.value, result)
                } else {
                    youtube_replace_query(url, "n", result)
                }
            });
        if let (Some(format), Some(url)) = (formats.get_mut(challenge.format_index), solved) {
            if let Some(object) = format.as_object_mut() {
                object.insert("url".to_owned(), serde_json::json!(url));
                let keep_todo = challenges.sig.iter().any(|sig| {
                    sig.format_index == challenge.format_index
                        && !solutions
                            .sig_specs
                            .contains_key(&sig.encrypted.chars().count())
                });
                if !keep_todo {
                    object.remove("rust_todo");
                }
            }
        } else {
            n_solved = false;
        }
    }
    (sig_solved, n_solved)
}
