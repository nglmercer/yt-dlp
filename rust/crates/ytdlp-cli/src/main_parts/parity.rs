#[derive(serde::Deserialize)]
struct ParityRequest {
    operation: String,
    input: serde_json::Value,
}

#[derive(serde::Serialize)]
struct ParityResponse {
    ok: bool,
    output: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct CliOptionRecord {
    group: String,
    aliases: Vec<String>,
    dest: Option<String>,
    action: Option<String>,
    #[serde(rename = "type")]
    value_type: Option<String>,
    nargs: Option<usize>,
    choices: Option<Vec<String>>,
}

fn print_help() {
    println!("yt-dlp-rs {MIGRATION_VERSION} (experimental Rust migration scaffold)");
    println!("Usage: yt-dlp-rs [OPTIONS] URL [URL...]");
    println!("       yt-dlp-rs --migration-status");
    println!("       yt-dlp-rs --format-bytes VALUE");
    println!("       yt-dlp-rs --parity-stdio");
    println!("       yt-dlp-rs --parse-args [OPTIONS] URL [URL...]");
    println!("       yt-dlp-rs --parse-configured-args [OPTIONS] URL [URL...]");
    println!("       yt-dlp-rs --native-request [OPTIONS] URL [URL...]");
    println!("       yt-dlp-rs --native-download [OPTIONS] URL [URL...]");
    println!("       yt-dlp-rs --native-postprocess [OPTIONS] FILE");
    println!("       yt-dlp-rs --extractor-info URL");
    println!();
    println!("Implemented CLI options in this migration slice:");
    println!("  -h, --help, --version, -q, -v, -s, -j, -J, -F");
    println!("  --proxy, --socket-timeout, --no-check-certificates");
    println!("  --user-agent, --referer, --add-headers");
    println!("  -f, --format, --all-formats, -S, --format-sort, --output");
    println!("  -g, --get-url, -e, --get-title, --get-id, --get-thumbnail, --get-duration");
    println!("  --write-info-json, --batch-file");
    println!("  --download-archive, --no-download-archive");
    println!("  --cookies, --no-cookies");
    println!("  --extract-audio, --audio-format, --remux-video, --ffmpeg-location");
    println!("  --no-playlist, --yes-playlist, --skip-download, --no-simulate");
    println!("  --native-request performs an opt-in raw request using the Rust network stack");
    println!("  --native-postprocess runs the opt-in FFmpeg postprocessor bridge");
    println!();
    println!("The executable is Rust-only; unported surfaces fail explicitly as TODO.");
}

fn request_model(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let object = input
        .as_object()
        .ok_or_else(|| "request_model input must be a JSON object".to_owned())?;
    let url = object
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "request_model input requires a string url".to_owned())?;

    let mut request = Request::new(url);
    if let Some(method) = object.get("method") {
        let method = method
            .as_str()
            .ok_or_else(|| "request_model method must be a string".to_owned())?;
        request
            .set_method(method)
            .map_err(|error| error.to_string())?;
    }
    if let Some(headers) = object.get("headers") {
        let headers = headers
            .as_object()
            .ok_or_else(|| "request_model headers must be a JSON object".to_owned())?;
        for (name, value) in headers {
            let value = value
                .as_str()
                .ok_or_else(|| "request_model header values must be strings".to_owned())?;
            request.headers_mut().set(name, value);
        }
    }
    let data = match object.get("data") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| "request_model data must be a string or null".to_owned())?
                .as_bytes()
                .to_vec(),
        ),
    };
    request.set_data(data);

    let data_hex = request.data().map(|data| {
        data.iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    });
    Ok(serde_json::json!({
        "url": request.url(),
        "method": request.method(),
        "data_hex": data_hex,
        "headers": request.headers().sensitive(),
    }))
}

fn cli_options(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let args = input
        .as_array()
        .ok_or_else(|| "cli_options input must be a JSON array".to_owned())?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "cli_options arguments must be strings".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    match parse_args(&args).map_err(|error| error.to_string())? {
        ParseResult::Options(options) => {
            serde_json::to_value(options).map_err(|error| error.to_string())
        }
        ParseResult::Help => Ok(serde_json::json!({"action": "help"})),
        ParseResult::Version => Ok(serde_json::json!({"action": "version"})),
    }
}

fn core_utils(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let object = input
        .as_object()
        .ok_or_else(|| "core_utils input must be a JSON object".to_owned())?;
    let function = object
        .get("function")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "core_utils input requires a function".to_owned())?;
    let output = match function {
        "determine_ext" => {
            let url = object.get("url").and_then(serde_json::Value::as_str);
            let default = object
                .get("default")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown_video");
            serde_json::json!(yt_dlp_core::determine_ext(url, default))
        }
        "determine_protocol" => {
            let value = object
                .get("info")
                .ok_or_else(|| "determine_protocol requires info".to_owned())?;
            let map = value
                .as_object()
                .ok_or_else(|| "determine_protocol info must be an object".to_owned())?;
            let mut info = InfoDict::new();
            for (key, value) in map {
                info.insert(key, value.clone());
            }
            serde_json::json!(
                yt_dlp_core::determine_protocol(&info).map_err(|error| error.to_string())?
            )
        }
        "int_or_none" => {
            let scale = object
                .get("scale")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(1);
            let invscale = object
                .get("invscale")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(1);
            let base = object
                .get("base")
                .and_then(serde_json::Value::as_u64)
                .map(|base| base as u32);
            serde_json::json!(yt_dlp_core::int_or_none(
                object.get("value"),
                scale,
                invscale,
                base,
            ))
        }
        "float_or_none" => {
            let scale = object
                .get("scale")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(1.0);
            let invscale = object
                .get("invscale")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(1.0);
            serde_json::json!(yt_dlp_core::float_or_none(
                object.get("value"),
                scale,
                invscale,
            ))
        }
        "str_or_none" => {
            let default = object.get("default").and_then(serde_json::Value::as_str);
            serde_json::json!(yt_dlp_core::str_or_none(object.get("value"), default))
        }
        "parse_iso8601" => {
            let value = object
                .get("value")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "parse_iso8601 requires a string value".to_owned())?;
            serde_json::json!(yt_dlp_core::parse_iso8601(value))
        }
        _ => return Err(format!("unsupported core utility: {function}")),
    };
    Ok(output)
}

fn cli_inventory() -> Result<serde_json::Value, String> {
    let records =
        serde_json::from_str::<Vec<CliOptionRecord>>(include_str!("../../data/options.json"))
            .map_err(|error| format!("invalid generated CLI manifest: {error}"))?;
    let aliases = records
        .iter()
        .flat_map(|record| record.aliases.iter())
        .collect::<Vec<_>>();
    let first_aliases = records
        .iter()
        .take(5)
        .map(|record| record.aliases.clone())
        .collect::<Vec<_>>();
    let last_aliases = records
        .iter()
        .rev()
        .take(5)
        .rev()
        .map(|record| record.aliases.clone())
        .collect::<Vec<_>>();
    let groups = records
        .iter()
        .map(|record| record.group.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let value_options = records
        .iter()
        .filter(|record| record.nargs.is_some() || record.value_type.is_some())
        .count();
    let callback_options = records
        .iter()
        .filter(|record| record.action.as_deref() == Some("callback"))
        .count();
    let destination_count = records
        .iter()
        .filter(|record| record.dest.is_some())
        .count();
    let choice_count = records
        .iter()
        .filter(|record| {
            record
                .choices
                .as_ref()
                .is_some_and(|choices| !choices.is_empty())
        })
        .count();
    Ok(serde_json::json!({
        "count": records.len(),
        "spelling_count": aliases.len(),
        "group_count": groups.len(),
        "value_option_count": value_options,
        "callback_option_count": callback_options,
        "destination_count": destination_count,
        "choice_option_count": choice_count,
        "first_aliases": first_aliases,
        "last_aliases": last_aliases,
    }))
}

fn extractor_inventory() -> Result<serde_json::Value, String> {
    let registry = ExtractorRegistry::generated().map_err(|error| error.to_string())?;
    let keys = registry
        .iter()
        .map(|extractor| extractor.descriptor().key.clone())
        .collect::<Vec<_>>();
    let names = registry
        .iter()
        .map(|extractor| extractor.descriptor().name.clone())
        .collect::<Vec<_>>();
    let working_count = registry
        .iter()
        .filter(|extractor| extractor.descriptor().working)
        .count();
    let pattern_count = registry
        .iter()
        .map(|extractor| extractor.pattern_count())
        .sum::<usize>();
    let embed_only_count = registry
        .iter()
        .filter(|extractor| extractor.pattern_count() == 0)
        .count();
    Ok(serde_json::json!({
        "count": registry.len(),
        "first_keys": keys.iter().take(5).collect::<Vec<_>>(),
        "last_keys": keys.iter().rev().take(5).rev().collect::<Vec<_>>(),
        "first_names": names.iter().take(5).collect::<Vec<_>>(),
        "last_names": names.iter().rev().take(5).rev().collect::<Vec<_>>(),
        "working_count": working_count,
        "pattern_count": pattern_count,
        "embed_only_count": embed_only_count,
    }))
}

fn downloader_manifests(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let object = input
        .as_object()
        .ok_or_else(|| "downloader_manifests input must be an object".to_owned())?;
    let kind = object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "downloader_manifests input requires kind".to_owned())?;
    let base_url = object
        .get("base_url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "downloader_manifests input requires base_url".to_owned())?;
    let body = object
        .get("body")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "downloader_manifests input requires body".to_owned())?;
    match kind {
        "hls" => {
            let playlist =
                parse_hls_playlist(base_url, body.as_bytes()).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "variant": playlist.variant,
                "segments": playlist.segments,
                "segment_ranges": playlist.segment_ranges.iter().map(|range| {
                    range.as_ref().map(|range| serde_json::json!({
                        "start": range.start,
                        "length": range.length,
                    }))
                }).collect::<Vec<_>>(),
            }))
        }
        "dash" => {
            let manifest =
                parse_dash_mpd(base_url, body.as_bytes()).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "segments": manifest.segments,
                "segment_ranges": manifest.segment_ranges.iter().map(|range| {
                    range.as_ref().map(|range| serde_json::json!({
                        "start": range.start,
                        "length": range.length,
                    }))
                }).collect::<Vec<_>>(),
            }))
        }
        _ => Err(format!("unsupported downloader manifest kind: {kind}")),
    }
}

fn parity_response(request: ParityRequest) -> ParityResponse {
    let result: Result<Option<serde_json::Value>, String> =
        match request.operation.as_str() {
            "format_bytes" => match request.input {
                serde_json::Value::Null => Ok(Some(serde_json::json!(format_bytes(None)))),
                serde_json::Value::Number(value) => value
                    .as_f64()
                    .map(|value| Some(serde_json::json!(format_bytes(Some(value)))))
                    .ok_or_else(|| "format_bytes input is not a finite number".to_owned()),
                _ => Err("format_bytes input must be a JSON number or null".to_owned()),
            },
            "parse_bytes" => match request.input.as_str() {
                Some(value) => Ok(yt_dlp_core::parse_bytes(value)
                    .map(|value| serde_json::json!(value.to_string()))),
                None => Err("parse_bytes input must be a string".to_owned()),
            },
            "parse_duration" => match request.input {
                serde_json::Value::String(value) => {
                    Ok(yt_dlp_core::parse_duration(&value).map(|value| serde_json::json!(value)))
                }
                // The reference behavior returns None for non-string inputs.
                _ => Ok(None),
            },
            "request_model" => request_model(request.input).map(Some),
            "cli_options" => cli_options(request.input).map(Some),
            "core_utils" => core_utils(request.input).map(Some),
            "cli_inventory" => cli_inventory().map(Some),
            "extractor_inventory" => extractor_inventory().map(Some),
            "downloader_manifests" => downloader_manifests(request.input).map(Some),
            operation => Err(format!("unsupported operation: {operation}")),
        };

    match result {
        Ok(output) => ParityResponse {
            ok: true,
            output,
            error: None,
        },
        Err(error) => ParityResponse {
            ok: false,
            output: None,
            error: Some(error),
        },
    }
}

fn run_parity_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());

    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) => match serde_json::from_str::<ParityRequest>(&line) {
                Ok(request) => parity_response(request),
                Err(error) => ParityResponse {
                    ok: false,
                    output: None,
                    error: Some(format!("invalid request: {error}")),
                },
            },
            Err(error) => ParityResponse {
                ok: false,
                output: None,
                error: Some(format!("could not read request: {error}")),
            },
        };

        serde_json::to_writer(&mut stdout, &response)?;
        writeln!(stdout)?;
        stdout.flush()?;
    }

    Ok(())
}

fn format_bytes_argument(raw: &str) -> Result<(), String> {
    let value = raw
        .parse::<f64>()
        .map_err(|error| format!("invalid byte count {raw:?}: {error}"))?;
    println!("{}", format_bytes(Some(value)));
    Ok(())
}

fn parse_args_argument(args: &[String]) -> Result<(), String> {
    parse_options_result(parse_args(args).map_err(|error| error.to_string())?)
}

fn parse_configured_args_argument(args: &[String]) -> Result<(), String> {
    parse_options_result(parse_configured_args(args).map_err(|error| error.to_string())?)
}

fn parse_options_result(result: ParseResult) -> Result<(), String> {
    match result {
        ParseResult::Options(options) => {
            serde_json::to_writer(io::stdout(), &options).map_err(|error| error.to_string())
        }
        ParseResult::Help => {
            print_help();
            Ok(())
        }
        ParseResult::Version => {
            println!("{MIGRATION_VERSION}");
            Ok(())
        }
    }
}
