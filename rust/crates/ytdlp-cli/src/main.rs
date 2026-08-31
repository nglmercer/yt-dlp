mod cli;

use std::env;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;

use cli::{ParseResult, parse_args, parse_configured_args, rust_supported_option_aliases};
use yt_dlp_core::{INITIAL_CAPABILITIES, InfoDict, MIGRATION_VERSION};
use yt_dlp_core::{format_bytes, render_output_template};
use yt_dlp_downloader::{DirectDownloader, DownloadOptions, DownloadResult};
use yt_dlp_extractor::{ExtractionContext, ExtractorRegistry, ExtractorResult};
use yt_dlp_javascript::{JavascriptRuntime, RuntimeKind};
use yt_dlp_networking::{CookieJar, Request, RequestDirector, Response};
use yt_dlp_postprocessor::{
    FfmpegExtractAudio, FfmpegRemuxer, FfmpegVideoConvertor, PostProcessOptions, PostProcessResult,
    PostProcessor,
};

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
        serde_json::from_str::<Vec<CliOptionRecord>>(include_str!("../data/options.json"))
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

fn response_json(response: &Response) -> serde_json::Value {
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| serde_json::json!([name, value]))
        .collect::<Vec<_>>();
    let body_hex = response
        .body()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    serde_json::json!({
        "url": response.url(),
        "status": response.status(),
        "reason": response.reason(),
        "headers": headers,
        "body_hex": body_hex,
    })
}

fn native_request_argument(args: &[String]) -> Result<(), String> {
    let result = parse_configured_args(args).map_err(|error| error.to_string())?;
    let ParseResult::Options(options) = result else {
        return parse_options_result(result);
    };
    let urls = native_input_urls(&options)?;

    let director = RequestDirector::native();
    let cookie_jar = CookieJar::new().shared();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for url in &urls {
        let request = options.request_for_url(url, cookie_jar.clone());
        let response = director.send(&request).map_err(|error| error.to_string())?;
        if options.dumpjson || options.dump_single_json {
            serde_json::to_writer(&mut stdout, &response_json(&response))
                .map_err(|error| error.to_string())?;
            writeln!(stdout).map_err(|error| error.to_string())?;
        } else {
            stdout
                .write_all(response.body())
                .map_err(|error| error.to_string())?;
        }
    }
    stdout.flush().map_err(|error| error.to_string())
}

fn direct_output_path(
    info: &InfoDict,
    options: &cli::CliOptions,
    selected_ext: Option<&str>,
) -> Result<PathBuf, String> {
    let mut output_info = info.clone();
    if let Some(selected_ext) = selected_ext {
        output_info.insert("ext", serde_json::json!(selected_ext));
    }
    if matches!(
        info.get("ext").and_then(serde_json::Value::as_str),
        Some("m3u8" | "mpd")
    ) {
        output_info.insert("ext", serde_json::json!("mp4"));
    }
    let template = options
        .outtmpl
        .get("default")
        .cloned()
        .unwrap_or_else(|| "%(id)s.%(ext)s".to_owned());
    render_output_template(&template, &output_info)
        .map(PathBuf::from)
        .map_err(|error| error.to_string())
}

fn format_records(info: &InfoDict) -> Vec<&serde_json::Value> {
    info.get("formats")
        .and_then(serde_json::Value::as_array)
        .map(|formats| formats.iter().collect())
        .unwrap_or_default()
}

fn select_download_format(
    info: &InfoDict,
    selector: Option<&str>,
) -> Result<(String, Option<String>), String> {
    let formats = format_records(info);
    let Some(selector) = selector else {
        let url = info
            .get("url")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                formats
                    .first()
                    .and_then(|format| format.get("url"))
                    .and_then(serde_json::Value::as_str)
            })
            .ok_or_else(|| "TODO: extractor returned no downloadable native URL".to_owned())?;
        let ext = info
            .get("ext")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        return Ok((url.to_owned(), ext));
    };

    let mut selected = None;
    for alternative in selector.split('/') {
        if matches!(alternative, "best" | "b" | "best*") {
            selected = formats.iter().find(|format| format.get("url").is_some());
        } else if matches!(alternative, "bestaudio" | "ba") {
            selected = formats.iter().find(|format| {
                format.get("vcodec").and_then(serde_json::Value::as_str) == Some("none")
            });
        } else if matches!(alternative, "bestvideo" | "bv") {
            selected = formats.iter().find(|format| {
                format.get("vcodec").and_then(serde_json::Value::as_str) != Some("none")
            });
        } else if matches!(alternative, "worst" | "w") {
            selected = formats
                .iter()
                .rev()
                .find(|format| format.get("url").is_some());
        } else if matches!(alternative, "worstaudio" | "wa") {
            selected = formats.iter().rev().find(|format| {
                format.get("vcodec").and_then(serde_json::Value::as_str) == Some("none")
            });
        } else if alternative == "all" {
            return Err("TODO: downloading all native formats is not implemented".to_owned());
        } else if alternative.contains('[')
            || alternative.contains('+')
            || alternative.contains(',')
            || alternative.contains('(')
        {
            return Err(format!(
                "TODO: native format selector syntax is not implemented: {alternative}"
            ));
        } else {
            selected = formats.iter().find(|format| {
                format.get("format_id").and_then(serde_json::Value::as_str) == Some(alternative)
                    || format.get("ext").and_then(serde_json::Value::as_str) == Some(alternative)
            });
        }
        if selected.is_some() {
            break;
        }
    }
    let format =
        selected.ok_or_else(|| format!("no native format matches selector: {selector}"))?;
    let url = format
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "selected native format has no URL".to_owned())?;
    let ext = format
        .get("ext")
        .and_then(serde_json::Value::as_str)
        .or_else(|| info.get("ext").and_then(serde_json::Value::as_str))
        .map(str::to_owned);
    Ok((url.to_owned(), ext))
}

fn native_postprocess_options(options: &cli::CliOptions, simulate: bool) -> PostProcessOptions {
    PostProcessOptions {
        ffmpeg_location: options.ffmpeg_location.as_deref().map(PathBuf::from),
        overwrite: !options.nopostoverwrites,
        keep_video: options.keepvideo,
        simulate: simulate || options.simulate == Some(true),
        extra_args: options.postprocessor_args.clone(),
    }
}

fn postprocess_rule_target(rule: &str) -> Option<String> {
    rule.split('/')
        .next()
        .and_then(|rule| {
            rule.rsplit_once('>')
                .map_or(Some(rule), |(_, target)| Some(target))
        })
        .map(str::trim)
        .filter(|target| !target.is_empty() && *target != "best")
        .map(str::to_owned)
}

fn run_native_postprocessor(
    info: &InfoDict,
    options: &cli::CliOptions,
    simulate: bool,
) -> Result<PostProcessResult, String> {
    let pp_options = native_postprocess_options(options, simulate);
    if options.extractaudio {
        let target = match options.audioformat.as_deref().unwrap_or("best") {
            "best" => "mp3",
            target => target,
        };
        let codec = match target {
            "mp3" => Some("libmp3lame"),
            "aac" | "m4a" => Some("aac"),
            "opus" => Some("libopus"),
            "vorbis" | "ogg" => Some("libvorbis"),
            "flac" => Some("flac"),
            "wav" => Some("pcm_s16le"),
            _ => None,
        }
        .map(str::to_owned);
        return FfmpegExtractAudio::new(target, codec)
            .map_err(|error| error.to_string())?
            .run(info, &pp_options)
            .map_err(|error| error.to_string());
    }
    if let Some(rule) = options.remuxvideo.as_deref() {
        let target = postprocess_rule_target(rule)
            .ok_or_else(|| "--remux-video requires a target format".to_owned())?;
        return FfmpegRemuxer::new(target)
            .map_err(|error| error.to_string())?
            .run(info, &pp_options)
            .map_err(|error| error.to_string());
    }
    if options.recodevideo.is_some() {
        let target = postprocess_rule_target(options.recodevideo.as_deref().unwrap_or_default())
            .ok_or_else(|| "--recode-video requires a target format".to_owned())?;
        return FfmpegVideoConvertor::new(target)
            .map_err(|error| error.to_string())?
            .run(info, &pp_options)
            .map_err(|error| error.to_string());
    }
    Err(
        "native postprocessing requires --extract-audio, --remux-video, or --recode-video"
            .to_owned(),
    )
}

fn postprocess_result_json(result: &PostProcessResult) -> serde_json::Value {
    serde_json::json!({
        "files_to_delete": result.files_to_delete,
        "info": result.info,
        "command": result.command.as_ref().map(|command| command.iter()
            .map(|argument| argument.to_string_lossy().into_owned()).collect::<Vec<_>>()),
        "simulated": result.simulated,
    })
}

fn native_postprocess_argument(args: &[String]) -> Result<(), String> {
    let result = parse_configured_args(args).map_err(|error| error.to_string())?;
    let ParseResult::Options(options) = result else {
        return parse_options_result(result);
    };
    if options.urls.len() != 1 {
        return Err("--native-postprocess requires exactly one input file".to_owned());
    }
    let input = PathBuf::from(&options.urls[0]);
    if !input.is_file() && options.simulate != Some(true) {
        return Err(format!("input file does not exist: {input:?}"));
    }
    let extension = input
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bin");
    let mut info = InfoDict::new();
    info.insert("filepath", serde_json::json!(input.to_string_lossy()));
    info.insert("ext", serde_json::json!(extension));
    info.insert(
        "id",
        serde_json::json!(
            input
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("video")
        ),
    );
    let result = run_native_postprocessor(&info, &options, false)?;
    if options.dumpjson || options.dump_single_json {
        println!(
            "{}",
            serde_json::to_string(&postprocess_result_json(&result))
                .map_err(|error| error.to_string())?
        );
    } else if let Some(path) = result.info.get_str("filepath") {
        println!(
            "[postprocess] {} -> {path}",
            result.info.get_str("ext").unwrap_or("media")
        );
    }
    Ok(())
}

fn download_result_json(result: &DownloadResult) -> serde_json::Value {
    serde_json::json!({
        "url": result.url,
        "status": result.status,
        "bytes": result.bytes,
        "path": result.path,
        "simulated": result.simulated,
        "fragments": result.fragments,
        "resumed": result.resumed,
    })
}

fn print_info_json(info: &InfoDict) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(info).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn print_formats(info: &InfoDict) {
    if let Some(formats) = info.get("formats").and_then(serde_json::Value::as_array) {
        for format in formats {
            let format_id = format
                .get("format_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let ext = format
                .get("ext")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let url = format
                .get("url")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<missing URL>");
            println!("{format_id}\t{ext}\t{url}");
        }
    } else if let Some(url) = info.get_str("url") {
        println!(
            "direct\t{}\t{url}",
            info.get_str("ext").unwrap_or("unknown")
        );
    }
}

fn print_requested_fields(info: &InfoDict, options: &cli::CliOptions, download_url: &str) -> bool {
    let mut printed = false;
    if options.geturl == Some(true) {
        println!("{download_url}");
        printed = true;
    }
    if options.gettitle == Some(true) {
        println!("{}", info.get_str("title").unwrap_or(""));
        printed = true;
    }
    if options.getid == Some(true) {
        println!("{}", info.get_str("id").unwrap_or(""));
        printed = true;
    }
    if options.getthumbnail == Some(true) {
        println!("{}", info.get_str("thumbnail").unwrap_or(""));
        printed = true;
    }
    if options.getduration == Some(true) {
        match info.get("duration") {
            Some(value) if !value.is_null() => println!("{value}"),
            _ => println!(),
        }
        printed = true;
    }
    printed
}

fn write_info_json(info: &InfoDict, output: &PathBuf) -> Result<PathBuf, String> {
    let info_path = output.with_extension("info.json");
    let bytes = serde_json::to_vec_pretty(info).map_err(|error| error.to_string())?;
    std::fs::write(&info_path, bytes)
        .map_err(|error| format!("could not write info JSON {info_path:?}: {error}"))?;
    Ok(info_path)
}

fn native_input_urls(options: &cli::CliOptions) -> Result<Vec<String>, String> {
    let mut urls = options.urls.clone();
    if let Some(batchfile) = options.batchfile.as_deref() {
        let contents = if batchfile == "-" {
            let mut contents = String::new();
            io::stdin()
                .read_to_string(&mut contents)
                .map_err(|error| format!("could not read batch file from stdin: {error}"))?;
            contents
        } else {
            std::fs::read_to_string(batchfile)
                .map_err(|error| format!("could not read batch file {batchfile:?}: {error}"))?
        };
        urls.extend(
            contents
                .lines()
                .map(str::trim)
                .filter(|url| !url.is_empty() && !url.starts_with('#'))
                .map(str::to_owned),
        );
    }
    if urls.is_empty() {
        return Err("at least one URL or --batch-file entry is required".to_owned());
    }
    Ok(urls)
}

fn native_playlist_indices(spec: Option<&str>, length: usize) -> Result<Vec<usize>, String> {
    if length == 0 {
        return Ok(Vec::new());
    }
    let Some(spec) = spec else {
        return Ok((0..length).collect());
    };
    let mut indices = Vec::new();
    for token in spec
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        if token == "-1" {
            indices.push(length - 1);
            continue;
        }
        if let Some((start, end)) = token.split_once('-') {
            let start = if start.is_empty() {
                1
            } else {
                start
                    .parse::<usize>()
                    .map_err(|error| format!("invalid --playlist-items range {token:?}: {error}"))?
            };
            let end = if end.is_empty() {
                length
            } else {
                end.parse::<usize>()
                    .map_err(|error| format!("invalid --playlist-items range {token:?}: {error}"))?
            };
            if start == 0 || end == 0 || start > end {
                return Err(format!("invalid --playlist-items range: {token}"));
            }
            for index in start..=end {
                if index <= length {
                    indices.push(index - 1);
                }
            }
            continue;
        }
        let index = token
            .parse::<usize>()
            .map_err(|error| format!("invalid --playlist-items value {token:?}: {error}"))?;
        if index == 0 {
            return Err("--playlist-items uses one-based positive indexes".to_owned());
        }
        if index <= length {
            indices.push(index - 1);
        }
    }
    indices.sort_unstable();
    indices.dedup();
    Ok(indices)
}

fn native_download_argument(args: &[String]) -> Result<(), String> {
    let result = parse_configured_args(args).map_err(|error| error.to_string())?;
    let ParseResult::Options(options) = result else {
        return parse_options_result(result);
    };
    let urls = native_input_urls(&options)?;
    let registry = ExtractorRegistry::generated().map_err(|error| error.to_string())?;
    let extraction_context = ExtractionContext::native();
    for url in urls {
        let mut per_url = options.clone();
        per_url.urls = vec![url];
        native_download_one(&per_url, &registry, &extraction_context)?;
    }
    Ok(())
}

fn native_download_one(
    options: &cli::CliOptions,
    registry: &ExtractorRegistry,
    extraction_context: &ExtractionContext,
) -> Result<(), String> {
    let url = options
        .urls
        .first()
        .ok_or_else(|| "native download requires one URL".to_owned())?;
    let extractor = registry
        .find(url)
        .ok_or_else(|| format!("no extractor matched URL: {url}"))?;
    let extraction = extractor
        .extract_with_context(url, extraction_context)
        .map_err(|error| error.to_string())?;
    match extraction {
        ExtractorResult::Single(info) => native_download_info(options, &info, extraction_context),
        ExtractorResult::Playlist { info, entries } => {
            native_download_playlist(options, info, entries, extraction_context)
        }
    }
}

fn native_download_playlist(
    options: &cli::CliOptions,
    mut info: InfoDict,
    entries: Vec<InfoDict>,
    extraction_context: &ExtractionContext,
) -> Result<(), String> {
    if options.noplaylist {
        return Err(
            "TODO: --no-playlist requires a single-item extractor view for this URL".to_owned(),
        );
    }
    if options.dump_single_json {
        info.insert("_type", serde_json::json!("playlist"));
        info.insert(
            "entries",
            serde_json::to_value(entries).map_err(|error| error.to_string())?,
        );
        return print_info_json(&info);
    }
    if options.listformats == Some(true) {
        for entry in &entries {
            print_formats(entry);
        }
        return Ok(());
    }
    let indices = native_playlist_indices(options.playlist_items.as_deref(), entries.len())?;
    for (position, index) in indices.into_iter().enumerate() {
        let mut entry = entries[index].clone();
        if let Some(title) = info.get("title") {
            entry.insert("playlist", title.clone());
        }
        if let Some(playlist_id) = info.get("id") {
            entry.insert("playlist_id", playlist_id.clone());
        }
        entry.insert("playlist_index", serde_json::json!(index + 1));
        if options.verbose {
            eprintln!(
                "[playlist] downloading entry {} of {}",
                position + 1,
                entries.len()
            );
        }
        native_download_info(options, &entry, extraction_context)?;
    }
    Ok(())
}

fn native_download_info(
    options: &cli::CliOptions,
    info: &InfoDict,
    extraction_context: &ExtractionContext,
) -> Result<(), String> {
    if options.dumpjson || options.dump_single_json {
        return print_info_json(&info);
    }
    if options.listformats == Some(true) {
        print_formats(&info);
        return Ok(());
    }
    let selector = options
        .format
        .as_deref()
        .or(options.extractaudio.then_some("bestaudio"));
    let (download_url, selected_ext) = select_download_format(&info, selector)?;
    let request = options.request_for_url(&download_url, extraction_context.cookie_jar().clone());
    let output = direct_output_path(info, options, selected_ext.as_deref())?;
    let requested_fields = print_requested_fields(info, options, &download_url);
    let info_path = if options.writeinfojson == Some(true) {
        Some(write_info_json(info, &output)?)
    } else {
        None
    };
    if requested_fields || options.skip_download {
        if let Some(info_path) = info_path.as_ref() {
            eprintln!("[info] {}", info_path.display());
        }
        return Ok(());
    }
    let download_options = DownloadOptions {
        simulate: options.simulate == Some(true),
        overwrite: options.overwrites != Some(false),
        resume: options.continue_dl && options.overwrites != Some(true),
        retries: options
            .retries
            .as_u64()
            .or_else(|| {
                options
                    .retries
                    .as_str()
                    .and_then(|value| value.parse().ok())
            })
            .unwrap_or(10) as usize,
        concurrent: usize::try_from(options.concurrent_fragments)
            .ok()
            .filter(|value| *value > 0)
            .unwrap_or(1),
    };
    let downloader = DirectDownloader::native();
    let result = match selected_ext
        .as_deref()
        .or_else(|| info.get("ext").and_then(serde_json::Value::as_str))
    {
        Some("m3u8") => downloader.download_hls(&request, Some(&output), &download_options),
        Some("mpd") => downloader.download_dash(&request, Some(&output), &download_options),
        _ if download_url
            .split('?')
            .next()
            .is_some_and(|url| url.ends_with(".m3u8")) =>
        {
            downloader.download_hls(&request, Some(&output), &download_options)
        }
        _ if download_url
            .split('?')
            .next()
            .is_some_and(|url| url.ends_with(".mpd")) =>
        {
            downloader.download_dash(&request, Some(&output), &download_options)
        }
        _ => downloader.download(&request, Some(&output), &download_options),
    }
    .map_err(|error| error.to_string())?;
    let postprocessed =
        if options.extractaudio || options.remuxvideo.is_some() || options.recodevideo.is_some() {
            let mut post_info = info.clone();
            if let Some(path) = result.path.as_ref() {
                post_info.insert("filepath", serde_json::json!(path.to_string_lossy()));
            } else {
                post_info.insert("filepath", serde_json::json!(output.to_string_lossy()));
            }
            Some(run_native_postprocessor(
                &post_info,
                options,
                result.simulated,
            )?)
        } else {
            None
        };
    if options.dumpjson || options.dump_single_json {
        let mut output_json = download_result_json(&result);
        if let Some(postprocessed) = postprocessed {
            output_json["postprocess"] = postprocess_result_json(&postprocessed);
        }
        println!(
            "{}",
            serde_json::to_string(&output_json).map_err(|error| error.to_string())?
        );
    } else if let Some(path) = result.path {
        if let Some(postprocessed) = postprocessed {
            println!(
                "[download] {} bytes -> {} -> {}",
                result.bytes,
                path.display(),
                postprocessed
                    .info
                    .get_str("filepath")
                    .unwrap_or("postprocessed")
            );
        } else {
            println!("[download] {} bytes -> {}", result.bytes, path.display());
        }
    } else {
        println!(
            "[download] simulated {} bytes from {}",
            result.bytes, result.url
        );
    }
    if let Some(info_path) = info_path {
        eprintln!("[info] {}", info_path.display());
    }
    Ok(())
}

fn extractor_info_argument(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("--extractor-info requires exactly one URL".to_owned());
    }
    let registry = ExtractorRegistry::generated().map_err(|error| error.to_string())?;
    let extractor = registry
        .find(&args[0])
        .ok_or_else(|| format!("no extractor matched URL: {}", args[0]))?;
    let descriptor = extractor.descriptor();
    let output = serde_json::json!({
        "key": descriptor.key,
        "name": descriptor.name,
        "working": descriptor.working,
        "source_module": descriptor.source_module,
        "source_class": descriptor.source_class,
        "pattern_count": extractor.pattern_count(),
        "native_matcher_count": extractor.native_matcher_count(),
        "matcher_error_count": extractor.matcher_error_count(),
        "native": extractor.is_native(),
        "implementation": if extractor.is_native() { "native" } else { "TODO" },
    });
    serde_json::to_writer(io::stdout(), &output).map_err(|error| error.to_string())
}

fn print_migration_status() -> Result<(), String> {
    println!("yt-dlp-rs {MIGRATION_VERSION}");
    println!("active backend: Rust-only");
    println!("Rust capabilities:");
    for capability in INITIAL_CAPABILITIES {
        println!("  {}: {:?}", capability.name, capability.mode);
    }
    let cli_manifest = cli_inventory()?;
    let records =
        serde_json::from_str::<Vec<CliOptionRecord>>(include_str!("../data/options.json"))
            .map_err(|error| format!("invalid generated CLI manifest: {error}"))?;
    let supported = rust_supported_option_aliases();
    let native_definitions = records
        .iter()
        .filter(|record| {
            record
                .aliases
                .iter()
                .any(|alias| supported.contains(&alias.as_str()))
        })
        .count();
    let native_spellings = records
        .iter()
        .flat_map(|record| record.aliases.iter())
        .filter(|alias| supported.contains(&alias.as_str()))
        .count();
    println!(
        "CLI inventory: {} definitions, {} spellings, {} groups",
        cli_manifest["count"], cli_manifest["spelling_count"], cli_manifest["group_count"],
    );
    println!(
        "CLI parser coverage: {} definitions, {} aliases; remaining options are TODO",
        native_definitions, native_spellings,
    );
    println!("JavaScript runtimes:");
    for kind in [
        RuntimeKind::Deno,
        RuntimeKind::Node,
        RuntimeKind::QuickJs,
        RuntimeKind::Bun,
    ] {
        match JavascriptRuntime::probe(kind, None) {
            Ok(Some(runtime)) => println!(
                "  {} {} at {} ({})",
                runtime.info().name,
                runtime.info().version,
                runtime.info().path.display(),
                if runtime.info().supported {
                    "supported"
                } else {
                    "unsupported version"
                }
            ),
            Ok(None) => println!("  {}: unavailable", kind.name()),
            Err(error) => println!("  {}: {error}", kind.name()),
        }
    }
    let registry = ExtractorRegistry::generated().map_err(|error| error.to_string())?;
    println!(
        "extractor inventory: {} entries, {} native-matchable, {} pattern errors",
        registry.len(),
        registry.native_matchable_count(),
        registry.pattern_error_count(),
    );
    for extractor in registry
        .iter()
        .filter(|extractor| extractor.matcher_error_count() > 0)
    {
        println!(
            "  pattern TODO: {} ({})",
            extractor.descriptor().key,
            extractor.matcher_errors().join("; "),
        );
    }
    println!(
        "extractor implementations: {} native, remaining extractors are TODO",
        registry.native_implementation_count(),
    );
    Ok(())
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("--help") | Some("-h") => print_help(),
        Some("--version") => println!("{MIGRATION_VERSION}"),
        Some("--format-bytes") => match env::args().nth(2) {
            Some(value) => {
                if let Err(error) = format_bytes_argument(&value) {
                    eprintln!("yt-dlp-rs: {error}");
                    std::process::exit(2);
                }
            }
            None => {
                eprintln!("yt-dlp-rs: --format-bytes requires a value");
                std::process::exit(2);
            }
        },
        Some("--parse-args") => {
            if let Err(error) = parse_args_argument(&args[1..]) {
                eprintln!("yt-dlp-rs: {error}");
                std::process::exit(2);
            }
        }
        Some("--parse-configured-args") => {
            if let Err(error) = parse_configured_args_argument(&args[1..]) {
                eprintln!("yt-dlp-rs: {error}");
                std::process::exit(2);
            }
        }
        Some("--parity-stdio") => {
            if let Err(error) = run_parity_stdio() {
                eprintln!("yt-dlp-rs: parity protocol failed: {error}");
                std::process::exit(1);
            }
        }
        Some("--native-request") => {
            if let Err(error) = native_request_argument(&args[1..]) {
                eprintln!("yt-dlp-rs: {error}");
                std::process::exit(2);
            }
        }
        Some("--native-download") => {
            if let Err(error) = native_download_argument(&args[1..]) {
                eprintln!("yt-dlp-rs: {error}");
                std::process::exit(2);
            }
        }
        Some("--native-postprocess") => {
            if let Err(error) = native_postprocess_argument(&args[1..]) {
                eprintln!("yt-dlp-rs: {error}");
                std::process::exit(2);
            }
        }
        Some("--extractor-info") => {
            if let Err(error) = extractor_info_argument(&args[1..]) {
                eprintln!("yt-dlp-rs: {error}");
                std::process::exit(2);
            }
        }
        Some("--migration-status") => {
            if let Err(error) = print_migration_status() {
                eprintln!("yt-dlp-rs: {error}");
                std::process::exit(1);
            }
        }
        _ => {
            if let Err(error) = native_download_argument(&args) {
                eprintln!("yt-dlp-rs: {error}");
                std::process::exit(2);
            }
        }
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;

    fn sample_info() -> InfoDict {
        let mut info = InfoDict::new();
        info.insert(
            "formats",
            serde_json::json!([
                {"format_id": "ogg", "ext": "ogg", "vcodec": "none", "url": "https://media.test/a.ogg"},
                {"format_id": "mp3", "ext": "mp3", "vcodec": "none", "url": "https://media.test/a.mp3"},
                {"format_id": "video", "ext": "mp4", "url": "https://media.test/a.mp4"}
            ]),
        );
        info
    }

    #[test]
    fn native_format_selection_supports_exact_audio_and_video_aliases() {
        let info = sample_info();
        assert_eq!(
            select_download_format(&info, Some("mp3")).unwrap().0,
            "https://media.test/a.mp3"
        );
        assert_eq!(
            select_download_format(&info, Some("bv")).unwrap().0,
            "https://media.test/a.mp4"
        );
        assert_eq!(
            select_download_format(&info, Some("ba")).unwrap().0,
            "https://media.test/a.ogg"
        );
    }

    #[test]
    fn complex_native_format_selection_is_explicitly_todo() {
        let error =
            select_download_format(&sample_info(), Some("bestvideo+bestaudio")).unwrap_err();
        assert!(error.starts_with("TODO:"));
    }

    #[test]
    fn native_input_urls_combines_command_line_and_batch_file_entries() {
        let path = std::env::temp_dir().join(format!(
            "yt-dlp-rs-batch-{}-{}.txt",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(
            &path,
            "\n# ignored\nhttps://example.test/one\n  https://example.test/two  \n",
        )
        .unwrap();
        let mut options = cli::CliOptions::default();
        options.urls.push("https://example.test/zero".to_owned());
        options.batchfile = Some(path.to_string_lossy().into_owned());

        let urls = native_input_urls(&options).unwrap();
        assert_eq!(
            urls,
            vec![
                "https://example.test/zero",
                "https://example.test/one",
                "https://example.test/two"
            ]
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_playlist_indices_supports_ranges_and_last_entry() {
        assert_eq!(
            native_playlist_indices(Some("1,3-4,-1"), 5).unwrap(),
            vec![0, 2, 3, 4]
        );
        assert!(native_playlist_indices(Some("0"), 5).is_err());
        assert!(native_playlist_indices(Some("4-2"), 5).is_err());
    }
}
