/// Native YouTube player-JavaScript inventory.
///
/// Mirrors `YoutubeIE` player handling in
/// `yt_dlp/extractor/youtube/_video.py` for the default configuration
/// (`player_js_variant = 'main'`, `player_js_version = 'actual'`). The module
/// resolves the player script URL from the page configuration, downloads the
/// script through the native request context, and extracts the signature
/// timestamp (`sts`). Challenge *solving* itself remains an explicit TODO
/// until the native player-JavaScript solver lands; this inventory names the
/// exact player revision so unhandled revisions fail with a revision-specific
/// TODO instead of a generic one.
const YOUTUBE_PLAYER_BASE: &str = "https://www.youtube.com";
const YOUTUBE_DEFAULT_PLAYER_VARIANT: &str = "main";

fn youtube_player_variant_path(variant: &str) -> Option<&'static str> {
    match variant {
        "main" => Some("player_ias.vflset/en_US/base.js"),
        "tcc" => Some("player_ias_tcc.vflset/en_US/base.js"),
        "tce" => Some("player_ias_tce.vflset/en_US/base.js"),
        "es5" => Some("player_es5.vflset/en_US/base.js"),
        "es6" => Some("player_es6.vflset/en_US/base.js"),
        "es6_tcc" => Some("player_es6_tcc.vflset/en_US/base.js"),
        "es6_tce" => Some("player_es6_tce.vflset/en_US/base.js"),
        "tv" => Some("tv-player-ias.vflset/tv-player-ias.js"),
        "tv_es6" => Some("tv-player-es6.vflset/tv-player-es6.js"),
        "phone" => Some("player-plasma-ias-phone-en_US.vflset/base.js"),
        "house" => Some("house_brand_player.vflset/en_US/base.js"),
        _ => None,
    }
}

fn youtube_player_path_after_id(player_url: &str, player_id: &str) -> Option<String> {
    let prefix = format!("/s/player/{player_id}/");
    let path = url::Url::parse(player_url)
        .ok()
        .map(|url| url.path().to_owned())
        .or_else(|| player_url.split(['?', '#']).next().map(str::to_owned))?;
    path.strip_prefix(&prefix).map(str::to_owned)
}

fn youtube_player_variant_for_path(player_path: &str) -> Option<&'static str> {
    for variant in [
        "main", "tcc", "tce", "es5", "es6", "es6_tcc", "es6_tce", "tv", "tv_es6", "phone", "house",
    ] {
        let Some(expected) = youtube_player_variant_path(variant) else {
            continue;
        };
        if expected == player_path {
            return Some(variant);
        }
        // Localised player paths embed the locale where the map has `en_US`.
        let pattern = format!(
            r#"\A{}\z"#,
            regex::escape(expected).replace("en_US", "[a-zA-Z0-9_]+")
        );
        let matches = Regex::new(&pattern)
            .ok()
            .and_then(|matcher| matcher.captures(player_path).ok().flatten())
            .is_some();
        if matches {
            return Some(variant);
        }
    }
    None
}

fn youtube_extract_player_id(player_url: &str) -> Option<String> {
    let marker = "/s/player/";
    let start = player_url.find(marker)? + marker.len();
    let remainder = &player_url[start..];
    let end = remainder.find('/').unwrap_or(remainder.len());
    let player_id = &remainder[..end];
    (!player_id.is_empty()
        && player_id.len() >= 8
        && player_id.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then(|| player_id.to_owned())
}

fn youtube_join_player_url(path_or_url: &str) -> String {
    if path_or_url.starts_with("https://") || path_or_url.starts_with("http://") {
        return path_or_url.to_owned();
    }
    if path_or_url.starts_with("//") {
        return format!("https:{path_or_url}");
    }
    if path_or_url.starts_with('/') {
        return format!("{YOUTUBE_PLAYER_BASE}{path_or_url}");
    }
    format!("{YOUTUBE_PLAYER_BASE}/{path_or_url}")
}

/// Mirror of `_construct_player_url` for the default variant. A page-provided
/// player URL that already uses the default variant is kept as-is; any other
/// variant is normalised to the default variant path for the same player ID.
pub(crate) fn youtube_construct_player_url(
    player_id: Option<&str>,
    player_url: Option<&str>,
) -> Result<String, ExtractorError> {
    let Some(player_url) = player_url else {
        let Some(player_id) = player_id.filter(|id| !id.is_empty()) else {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "_construct_player_url must take one of player_id or player_url",
            ));
        };
        let path = youtube_player_variant_path(YOUTUBE_DEFAULT_PLAYER_VARIANT)
            .expect("default player variant is mapped");
        return Ok(format!("{YOUTUBE_PLAYER_BASE}/s/player/{player_id}/{path}"));
    };
    let player_id = player_id
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .or_else(|| youtube_extract_player_id(player_url))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Cannot identify player {player_url:?}"),
            )
        })?;
    let actual_variant = youtube_player_path_after_id(player_url, &player_id)
        .as_deref()
        .and_then(youtube_player_variant_for_path);
    if actual_variant == Some(YOUTUBE_DEFAULT_PLAYER_VARIANT) {
        return Ok(youtube_join_player_url(player_url));
    }
    let path = youtube_player_variant_path(YOUTUBE_DEFAULT_PLAYER_VARIANT)
        .expect("default player variant is mapped");
    Ok(format!("{YOUTUBE_PLAYER_BASE}/s/player/{player_id}/{path}"))
}

/// Mirror of `_extract_player_url`: read `PLAYER_JS_URL`, falling back to
/// `WEB_PLAYER_CONTEXT_CONFIGS[*].jsUrl`, then normalise the variant.
pub(crate) fn youtube_extract_player_url(ytcfg: &serde_json::Value) -> Option<String> {
    if let Some(player_url) = ytcfg
        .get("PLAYER_JS_URL")
        .and_then(serde_json::Value::as_str)
    {
        return youtube_construct_player_url(None, Some(player_url)).ok();
    }
    let configs = ytcfg.get("WEB_PLAYER_CONTEXT_CONFIGS")?;
    let configs = configs.as_object()?;
    for config in configs.values() {
        if let Some(player_url) = config.get("jsUrl").and_then(serde_json::Value::as_str) {
            if let Ok(url) = youtube_construct_player_url(None, Some(player_url)) {
                return Some(url);
            }
        }
    }
    None
}

/// Mirror of `_player_js_cache_key`: `{player_id}-{variant}` with unknown
/// variants sanitised to filesystem-safe tokens.
pub(crate) fn youtube_player_js_cache_key(player_url: &str) -> Option<String> {
    let player_id = youtube_extract_player_id(player_url)?;
    let variant = youtube_player_path_after_id(player_url, &player_id)
        .as_deref()
        .and_then(youtube_player_variant_for_path)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let path = youtube_player_path_after_id(player_url, &player_id).unwrap_or_default();
            let path = path.strip_suffix(".js").unwrap_or(&path);
            path.bytes()
                .map(|byte| {
                    if byte.is_ascii_alphanumeric() {
                        byte as char
                    } else {
                        '_'
                    }
                })
                .collect()
        });
    Some(format!("{player_id}-{variant}"))
}

/// Mirror of the `sts` half of `_extract_signature_timestamp`: prefer the page
/// configuration value, then the player script body.
pub(crate) fn youtube_signature_timestamp(
    ytcfg: &serde_json::Value,
    player_js: Option<&str>,
) -> Option<i64> {
    if let Some(sts) = ytcfg.get("STS").and_then(|sts| {
        sts.as_i64()
            .or_else(|| sts.as_str().and_then(|sts| sts.parse::<i64>().ok()))
    }) {
        return Some(sts);
    }
    let code = player_js?;
    Regex::new(r#"(?:signatureTimestamp|sts)\s*:\s*(?P<sts>[0-9]{5})"#)
        .ok()?
        .captures(code)
        .ok()
        .flatten()?
        .name("sts")?
        .as_str()
        .parse()
        .ok()
}

fn youtube_player_request(url: &str) -> Request {
    let mut request = Request::new(url);
    request
        .headers_mut()
        .set("User-Agent", YOUTUBE_DEFAULT_USER_AGENT);
    request.headers_mut().set("Accept", "*/*");
    request
}

struct YoutubePlayerInventory {
    player_id: String,
    sts: Option<i64>,
    /// The downloaded player script, kept for challenge solving.
    script: Option<String>,
}

/// Resolve the player script URL from the page configuration and download the
/// script best-effort. A missing player URL is an extraction error (mirroring
/// the fatal path); a failed script download only loses the `sts` fallback
/// and keeps extraction going with revision-specific TODOs.
fn youtube_resolve_player(
    context: &ExtractionContext,
    ytcfg: &serde_json::Value,
) -> Result<YoutubePlayerInventory, ExtractorError> {
    let url = youtube_extract_player_url(ytcfg).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            "TODO: YouTube player JavaScript URL was not found in the page configuration",
        )
    })?;
    let player_id = youtube_extract_player_id(&url).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Cannot identify player {url:?}"),
        )
    })?;
    let player_js = context
        .request(&youtube_player_request(&url))
        .ok()
        .map(|response| String::from_utf8_lossy(response.body()).into_owned());
    let sts = youtube_signature_timestamp(ytcfg, player_js.as_deref());
    Ok(YoutubePlayerInventory {
        player_id,
        sts,
        script: player_js,
    })
}

fn youtube_player_revision_label(player: &YoutubePlayerInventory) -> String {
    match player.sts {
        Some(sts) => format!("player {}, sts {sts}", player.player_id),
        None => format!("player {}, sts unknown", player.player_id),
    }
}

/// Rewrite generic solver TODOs to name the concrete player revision, per the
/// readiness gate: unhandled revisions must surface a revision-specific TODO.
fn youtube_annotate_challenge_todos(
    todos: Vec<String>,
    player: Option<&YoutubePlayerInventory>,
) -> Vec<String> {
    let Some(player) = player else {
        return todos;
    };
    let label = youtube_player_revision_label(player);
    todos
        .into_iter()
        .map(|todo| {
            if todo.contains("player-JavaScript solver") || todo.contains("challenge solver") {
                format!("{todo} ({label})")
            } else {
                todo
            }
        })
        .collect()
}
