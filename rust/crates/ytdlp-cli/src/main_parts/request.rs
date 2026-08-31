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
