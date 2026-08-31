mod cli;

use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use cli::{ParseResult, parse_args, parse_configured_args};
use yt_dlp_core::format_bytes;
use yt_dlp_core::{INITIAL_CAPABILITIES, MIGRATION_VERSION};
use yt_dlp_downloader::{DirectDownloader, DownloadOptions, DownloadResult};
use yt_dlp_extractor::ExtractorRegistry;
use yt_dlp_networking::{CookieJar, Request, RequestDirector, Response};

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

fn print_help() {
    println!("yt-dlp-rs {MIGRATION_VERSION} (experimental Rust migration scaffold)");
    println!("Usage: yt-dlp-rs [OPTIONS] URL [URL...]");
    println!("       yt-dlp-rs --migration-status");
    println!("       yt-dlp-rs --format-bytes VALUE");
    println!("       yt-dlp-rs --parity-stdio");
    println!("       yt-dlp-rs --parse-args [OPTIONS] URL [URL...]");
    println!("       yt-dlp-rs --parse-configured-args [OPTIONS] URL [URL...]");
    println!("       yt-dlp-rs --native-request [OPTIONS] URL [URL...]");
    println!("       yt-dlp-rs --native-download [OPTIONS] URL");
    println!("       yt-dlp-rs --extractor-info URL");
    println!();
    println!("Implemented CLI options in this migration slice:");
    println!("  -h, --help, --version, -q, -v, -s, -j, -J, -F");
    println!("  --proxy, --socket-timeout, --no-check-certificates");
    println!("  --user-agent, --referer, --add-headers");
    println!("  -f, --format, --all-formats, -S, --format-sort, --output");
    println!("  --no-playlist, --yes-playlist, --skip-download, --no-simulate");
    println!("  --native-request performs an opt-in raw request using the Rust network stack");
    println!();
    println!("The Python yt-dlp implementation remains the active downloader.");
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
                // Python returns None for non-string inputs as well as None.
                _ => Ok(None),
            },
            "request_model" => request_model(request.input).map(Some),
            "cli_options" => cli_options(request.input).map(Some),
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
    if options.urls.is_empty() {
        return Err("--native-request requires at least one URL".to_owned());
    }
    if options.batchfile.is_some() {
        return Err("--native-request does not support --batch-file yet".to_owned());
    }

    let director = RequestDirector::native();
    let cookie_jar = CookieJar::new().shared();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for url in &options.urls {
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

fn direct_output_path(info: &yt_dlp_core::InfoDict, options: &cli::CliOptions) -> PathBuf {
    let id = info
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("download");
    let ext = info
        .get("ext")
        .and_then(serde_json::Value::as_str)
        .map_or("bin", |extension| {
            if matches!(extension, "m3u8" | "mpd") {
                "mp4"
            } else {
                extension
            }
        });
    let template = options
        .outtmpl
        .get("default")
        .cloned()
        .unwrap_or_else(|| "%(id)s.%(ext)s".to_owned());
    PathBuf::from(
        template
            .replace("%(id)s", id)
            .replace("%(title)s", id)
            .replace("%(ext)s", ext),
    )
}

fn download_result_json(result: &DownloadResult) -> serde_json::Value {
    serde_json::json!({
        "url": result.url,
        "status": result.status,
        "bytes": result.bytes,
        "path": result.path,
        "simulated": result.simulated,
        "fragments": result.fragments,
    })
}

fn native_download_argument(args: &[String]) -> Result<(), String> {
    let result = parse_configured_args(args).map_err(|error| error.to_string())?;
    let ParseResult::Options(options) = result else {
        return parse_options_result(result);
    };
    if options.urls.len() != 1 {
        return Err("--native-download currently requires exactly one URL".to_owned());
    }
    if options.batchfile.is_some() {
        return Err("--native-download does not support --batch-file yet".to_owned());
    }
    if options.skip_download {
        if options.dumpjson || options.dump_single_json {
            println!("{{\"skipped\":true}}");
        }
        return Ok(());
    }

    let url = &options.urls[0];
    let registry = ExtractorRegistry::generated().map_err(|error| error.to_string())?;
    let extractor = registry
        .find(url)
        .ok_or_else(|| format!("no extractor matched URL: {url}"))?;
    if extractor.descriptor().key != "GenericIE" {
        return Err(format!(
            "native direct download is not implemented for extractor {}",
            extractor.descriptor().name
        ));
    }
    let info = extractor.extract(url).map_err(|error| error.to_string())?;
    let request = options.request_for_url(url, CookieJar::new().shared());
    let output = direct_output_path(&info, &options);
    let download_options = DownloadOptions {
        simulate: options.simulate == Some(true),
        overwrite: true,
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
    };
    let downloader = DirectDownloader::native();
    let result = match info.get("ext").and_then(serde_json::Value::as_str) {
        Some("m3u8") => downloader.download_hls(&request, Some(&output), &download_options),
        Some("mpd") => downloader.download_dash(&request, Some(&output), &download_options),
        _ => downloader.download(&request, Some(&output), &download_options),
    }
    .map_err(|error| error.to_string())?;
    if options.dumpjson || options.dump_single_json {
        println!(
            "{}",
            serde_json::to_string(&download_result_json(&result))
                .map_err(|error| error.to_string())?
        );
    } else if let Some(path) = result.path {
        println!("[download] {} bytes -> {}", result.bytes, path.display());
    } else {
        println!(
            "[download] simulated {} bytes from {}",
            result.bytes, result.url
        );
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
    });
    serde_json::to_writer(io::stdout(), &output).map_err(|error| error.to_string())
}

fn print_migration_status() -> Result<(), String> {
    println!("yt-dlp-rs {MIGRATION_VERSION}");
    println!("active backend: Python compatibility");
    println!("Rust capabilities:");
    for capability in INITIAL_CAPABILITIES {
        println!("  {}: {:?}", capability.name, capability.mode);
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
            "  pattern compatibility: {} ({})",
            extractor.descriptor().key,
            extractor.matcher_errors().join("; "),
        );
    }
    println!(
        "extractor implementations: {} native, compatibility fallback pending",
        registry.native_implementation_count(),
    );
    Ok(())
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("--help") | Some("-h") => print_help(),
        Some("--version") => println!("yt-dlp-rs {MIGRATION_VERSION}"),
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
            eprintln!("yt-dlp-rs: Rust migration scaffold; no download features are active yet");
            eprintln!("Try --migration-status, --parse-args, or --help.");
            std::process::exit(2);
        }
    }
}
