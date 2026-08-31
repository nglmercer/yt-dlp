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
