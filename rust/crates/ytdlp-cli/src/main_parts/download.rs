fn native_download_argument(args: &[String]) -> Result<(), String> {
    let result = parse_configured_args(args).map_err(|error| error.to_string())?;
    let ParseResult::Options(options) = result else {
        return parse_options_result(result);
    };
    // Mirrors the `allow_unplayable_formats` constructor warning: exact
    // message body, Rust `[warning]` prefix per local convention.
    if options.allow_unplayable_formats {
        eprintln!(
            "[warning] You have asked for UNPLAYABLE formats to be listed/downloaded. \
             This is a developer option intended for debugging.\n         \
             If you experience any issues while using this option, DO NOT open a bug report"
        );
    }
    let urls = native_input_urls(&options)?;
    let registry = ExtractorRegistry::generated().map_err(|error| error.to_string())?;
    let extraction_context =
        ExtractionContext::native().with_extractor_args(options.extractor_args.clone());
    let cookie_path = options.cookiefile.as_deref().map(PathBuf::from);
    if let Some(path) = cookie_path.as_deref() {
        extraction_context
            .cookie_jar()
            .lock()
            .map_err(|_| "cookie jar lock poisoned".to_owned())?
            .load_netscape_file(path)
            .map_err(|error| error.to_string())?;
    }
    let archive_path = options.download_archive.as_deref().map(PathBuf::from);
    let mut archive =
        DownloadArchive::open(archive_path.as_deref()).map_err(|error| error.to_string())?;
    for url in urls {
        let mut per_url = options.clone();
        per_url.urls = vec![url];
        native_download_one(&per_url, &registry, &extraction_context, &mut archive)?;
    }
    if let Some(path) = cookie_path.as_deref() {
        extraction_context
            .cookie_jar()
            .lock()
            .map_err(|_| "cookie jar lock poisoned".to_owned())?
            .save_netscape_file(path)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn native_download_one(
    options: &cli::CliOptions,
    registry: &ExtractorRegistry,
    extraction_context: &ExtractionContext,
    archive: &mut DownloadArchive,
) -> Result<(), String> {
    native_download_one_with_redirect_depth(options, registry, extraction_context, archive, 0)
}

fn native_download_one_with_redirect_depth(
    options: &cli::CliOptions,
    registry: &ExtractorRegistry,
    extraction_context: &ExtractionContext,
    archive: &mut DownloadArchive,
    redirect_depth: usize,
) -> Result<(), String> {
    if redirect_depth >= 20 {
        return Err("TODO: native extractor redirect chain exceeded 20 levels".to_owned());
    }
    let url = options
        .urls
        .first()
        .ok_or_else(|| "native download requires one URL".to_owned())?;
    let extractor = registry
        .find(url)
        .ok_or_else(|| format!("no extractor matched URL: {url}"))?;
    let extractor_key = extractor.descriptor().key.clone();
    let extraction = extractor
        .extract_with_context(url, extraction_context)
        .map_err(|error| error.to_string())?;
    match extraction {
        ExtractorResult::Single(info) => {
            let info = if native_is_url_result(&info) {
                native_resolve_playlist_entry(registry, extraction_context, &info, 0)?
            } else {
                info
            };
            native_download_info(
                options,
                &info,
                extraction_context,
                archive,
                Some(&extractor_key),
            )
        }
        ExtractorResult::Redirect { url, .. } => {
            let mut redirected = options.clone();
            redirected.urls = vec![url];
            native_download_one_with_redirect_depth(
                &redirected,
                registry,
                extraction_context,
                archive,
                redirect_depth + 1,
            )
        }
        ExtractorResult::Playlist { info, entries } => native_download_playlist(
            options,
            registry,
            info,
            entries,
            extraction_context,
            archive,
            Some(&extractor_key),
        ),
    }
}

fn native_download_playlist(
    options: &cli::CliOptions,
    registry: &ExtractorRegistry,
    mut info: InfoDict,
    entries: Vec<InfoDict>,
    extraction_context: &ExtractionContext,
    archive: &mut DownloadArchive,
    fallback_extractor: Option<&str>,
) -> Result<(), String> {
    let entries = entries
        .iter()
        .map(|entry| native_resolve_playlist_entry(registry, extraction_context, entry, 0))
        .collect::<Result<Vec<_>, _>>()?;
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
        native_download_info(
            options,
            &entry,
            extraction_context,
            archive,
            fallback_extractor,
        )?;
    }
    Ok(())
}

fn native_resolve_playlist_entry(
    registry: &ExtractorRegistry,
    extraction_context: &ExtractionContext,
    entry: &InfoDict,
    redirect_depth: usize,
) -> Result<InfoDict, String> {
    if entry.get_str("_type") != Some("url")
        && entry.get_str("_type") != Some("url_transparent")
    {
        return Ok(entry.clone());
    }
    if redirect_depth >= 20 {
        return Err("TODO: native playlist URL-result chain exceeded 20 levels".to_owned());
    }
    let target_url = entry
        .get_str("url")
        .ok_or_else(|| "TODO: native playlist URL result has no target URL".to_owned())?;
    let extractor = registry
        .find(target_url)
        .ok_or_else(|| format!("no extractor matched playlist entry URL: {target_url}"))?;
    let extraction = extractor
        .extract_with_context(target_url, extraction_context)
        .map_err(|error| error.to_string())?;
    let resolved = match extraction {
        ExtractorResult::Single(info) => {
            if native_is_url_result(&info) {
                native_resolve_playlist_entry(
                    registry,
                    extraction_context,
                    &info,
                    redirect_depth + 1,
                )?
            } else {
                info
            }
        }
        ExtractorResult::Redirect { url, .. } => {
            let mut redirect = InfoDict::new();
            redirect.insert("_type", serde_json::json!("url"));
            redirect.insert("url", serde_json::json!(url));
            native_resolve_playlist_entry(
                registry,
                extraction_context,
                &redirect,
                redirect_depth + 1,
            )?
        }
        ExtractorResult::Playlist { .. } => {
            return Err(
                "TODO: nested native playlists inside URL results are not implemented".to_owned(),
            );
        }
    };
    Ok(native_merge_playlist_entry_metadata(entry, resolved))
}

fn native_is_url_result(info: &InfoDict) -> bool {
    matches!(
        info.get_str("_type"),
        Some("url" | "url_transparent")
    )
}

fn native_merge_playlist_entry_metadata(source: &InfoDict, mut resolved: InfoDict) -> InfoDict {
    let transparent = source.get_str("_type") == Some("url_transparent");
    for (key, value) in source.iter() {
        if matches!(key, "_type" | "url" | "ie_key") {
            continue;
        }
        if transparent || !resolved.contains_key(key) {
            resolved.insert(key, value.clone());
        }
    }
    resolved
}

/// Print the interactive format prompt without a trailing newline, mirroring
/// the `to_screen(..., skip_eol=True)` prompt in `process_video_result`.
fn print_format_prompt() {
    print!("\nEnter format selector (Press ENTER for default, or Ctrl+C to quit): ");
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

/// Read one interactive reply line. `Ok(0)` (EOF) and read errors yield
/// `None`, which aborts selection cleanly; only the `\n` terminator is
/// stripped, mirroring `input()`.
fn read_format_reply() -> Option<String> {
    let mut line = String::new();
    match std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut line) {
        Ok(0) => None,
        Ok(_) => Some(line.strip_suffix('\n').unwrap_or(&line).to_owned()),
        Err(_) => None,
    }
}

fn native_download_info(
    options: &cli::CliOptions,
    info: &InfoDict,
    extraction_context: &ExtractionContext,
    archive: &mut DownloadArchive,
    fallback_extractor: Option<&str>,
) -> Result<(), String> {
    if options.dumpjson || options.dump_single_json {
        return print_info_json(&info);
    }
    if options.listformats == Some(true) {
        print_sorted_formats(info, options);
        return Ok(());
    }
    // Mirrors `process_video_result`: `-f -` lists the table, then prompts
    // until a reply selects something.
    let selections = if options.format.as_deref() == Some("-") {
        print_sorted_formats(info, options);
        select_interactive_downloads(info, options, &print_format_prompt, &mut read_format_reply)?
    } else {
        select_native_downloads(info, options.format.as_deref(), options)?
    };
    if selections.is_empty() {
        return Err("Requested format is not available".to_owned());
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
    // Requested-fields and info-JSON pass, like one `process_info` per
    // selected format in the oracle.
    let mut requested_fields = false;
    let mut info_paths = Vec::new();
    for selection in &selections {
        let (view, primary_url) = selection_info_view(info, selection);
        requested_fields |= print_requested_fields(&view, options, &primary_url);
        if options.writeinfojson == Some(true) {
            let output = selection_output_path(options, info, selection)?;
            info_paths.push(write_info_json(&view, &output)?);
        }
    }
    if requested_fields || options.skip_download {
        for info_path in &info_paths {
            eprintln!("[info] {}", info_path.display());
        }
        return Ok(());
    }
    for selection in &selections {
        match selection {
            NativeSelection::Single(format) => native_download_single_format(
                options,
                info,
                format,
                extraction_context,
                archive,
                fallback_extractor,
                &download_options,
                &downloader,
            )?,
            NativeSelection::Merged(merged) => native_download_merged_format(
                options,
                info,
                merged,
                extraction_context,
                archive,
                fallback_extractor,
                &download_options,
                &downloader,
            )?,
        }
    }
    for info_path in &info_paths {
        eprintln!("[info] {}", info_path.display());
    }
    Ok(())
}

/// The info view and primary URL used for requested-fields printing and
/// info-JSON output of one selection.
fn selection_info_view(info: &InfoDict, selection: &NativeSelection) -> (InfoDict, String) {
    match selection {
        NativeSelection::Single(format) => (
            info.clone(),
            format
                .get("url")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned(),
        ),
        NativeSelection::Merged(merged) => {
            let mut view = info.clone();
            if let Some(fields) = merged.as_object() {
                for (key, value) in fields {
                    view.insert(key, value.clone());
                }
            }
            let primary_url = merged
                .get("requested_formats")
                .and_then(serde_json::Value::as_array)
                .and_then(|parts| parts.first())
                .and_then(|part| part.get("url"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            (view, primary_url)
        }
    }
}

fn selection_output_path(
    options: &cli::CliOptions,
    info: &InfoDict,
    selection: &NativeSelection,
) -> Result<PathBuf, String> {
    match selection {
        NativeSelection::Single(format) => {
            let ext = format
                .get("ext")
                .and_then(serde_json::Value::as_str)
                .or_else(|| info.get("ext").and_then(serde_json::Value::as_str));
            direct_output_path(info, options, ext)
        }
        NativeSelection::Merged(merged) => {
            let mut view = info.clone();
            if let Some(fields) = merged.as_object() {
                for (key, value) in fields {
                    view.insert(key, value.clone());
                }
            }
            let ext = merged.get("ext").and_then(serde_json::Value::as_str);
            direct_output_path(&view, options, ext)
        }
    }
}

/// The protocol used to dispatch one format: its own declaration, the info
/// declaration, or the matching info format, like the oracle's per-format
/// downloader choice.
fn native_format_protocol(
    info: &InfoDict,
    format: &serde_json::Value,
    download_url: &str,
) -> Option<String> {
    format
        .get("protocol")
        .and_then(serde_json::Value::as_str)
        .or_else(|| info.get("protocol").and_then(serde_json::Value::as_str))
        .or_else(|| {
            format_records(info).into_iter().find_map(|record| {
                (record.get("url").and_then(serde_json::Value::as_str) == Some(download_url))
                    .then(|| record.get("protocol").and_then(serde_json::Value::as_str))
                    .flatten()
            })
        })
        .map(str::to_owned)
}

fn native_dispatch_download(
    downloader: &DirectDownloader,
    request: &Request,
    download_url: &str,
    declared_ext: Option<&str>,
    declared_protocol: Option<&str>,
    output: &std::path::Path,
    download_options: &DownloadOptions,
) -> Result<DownloadResult, String> {
    if let Some(todo) =
        native_protocol_todo(download_url, declared_ext, declared_protocol.as_deref())
    {
        return Err(todo);
    }
    let result = match declared_ext {
        Some("m3u8") => downloader.download_hls(request, Some(output), download_options),
        Some("mpd") => downloader.download_dash(request, Some(output), download_options),
        _ if declared_protocol
            .as_deref()
            .is_some_and(native_hls_protocol) =>
        {
            downloader.download_hls(request, Some(output), download_options)
        }
        _ if declared_protocol
            .as_deref()
            .is_some_and(native_dash_protocol) =>
        {
            downloader.download_dash(request, Some(output), download_options)
        }
        _ if native_url_ends_with(download_url, ".m3u8") => {
            downloader.download_hls(request, Some(output), download_options)
        }
        _ if native_url_ends_with(download_url, ".mpd") => {
            downloader.download_dash(request, Some(output), download_options)
        }
        _ => downloader.download(request, Some(output), download_options),
    }
    .map_err(|error| error.to_string())?;
    Ok(result)
}

fn native_download_single_format(
    options: &cli::CliOptions,
    info: &InfoDict,
    format: &serde_json::Value,
    extraction_context: &ExtractionContext,
    archive: &mut DownloadArchive,
    fallback_extractor: Option<&str>,
    download_options: &DownloadOptions,
    downloader: &DirectDownloader,
) -> Result<(), String> {
    let selected = selected_format_details(format, info)?;
    let download_url = selected.url;
    let selected_ext = selected.ext;
    let mut request =
        options.request_for_url(&download_url, extraction_context.cookie_jar().clone());
    native_apply_info_http_headers(&mut request, info)?;
    if let Some(extra_param) = selected.extra_param_to_segment_url {
        request.extensions_mut().insert(
            "extra_param_to_segment_url".to_owned(),
            serde_json::json!(extra_param),
        );
    }
    let output = direct_output_path(info, options, selected_ext.as_deref())?;
    let declared_ext = selected_ext
        .as_deref()
        .or_else(|| info.get("ext").and_then(serde_json::Value::as_str));
    let declared_protocol = native_format_protocol(info, format, &download_url);
    if archive.contains_info(info, fallback_extractor) {
        if !options.quiet.unwrap_or(false) {
            eprintln!(
                "[download] {} is already present in the download archive",
                archive
                    .id_for_info(info, fallback_extractor)
                    .unwrap_or_else(|| "item".to_owned())
            );
        }
        return Ok(());
    }
    let result = native_dispatch_download(
        downloader,
        &request,
        &download_url,
        declared_ext,
        declared_protocol.as_deref(),
        &output,
        download_options,
    )?;
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
    if !result.simulated && !options.skip_download {
        archive
            .record_info(info, fallback_extractor)
            .map_err(|error| error.to_string())?;
    }
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
    Ok(())
}

/// Download every `requested_formats` part into `f<id>` files and merge them
/// with FFmpeg, mirroring the oracle's merged-format download path.
fn native_download_merged_format(
    options: &cli::CliOptions,
    info: &InfoDict,
    merged: &serde_json::Value,
    extraction_context: &ExtractionContext,
    archive: &mut DownloadArchive,
    fallback_extractor: Option<&str>,
    download_options: &DownloadOptions,
    downloader: &DirectDownloader,
) -> Result<(), String> {
    let parts = merged
        .get("requested_formats")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "merged native format has no requested formats".to_owned())?;
    if parts.is_empty() {
        return Err("merged native format has no requested formats".to_owned());
    }
    let merged_ext = merged.get("ext").and_then(serde_json::Value::as_str);
    let mut merged_info = info.clone();
    if let Some(fields) = merged.as_object() {
        for (key, value) in fields {
            merged_info.insert(key, value.clone());
        }
    }
    let output = direct_output_path(&merged_info, options, merged_ext)?;
    if archive.contains_info(&merged_info, fallback_extractor) {
        if !options.quiet.unwrap_or(false) {
            eprintln!(
                "[download] {} is already present in the download archive",
                archive
                    .id_for_info(&merged_info, fallback_extractor)
                    .unwrap_or_else(|| "item".to_owned())
            );
        }
        return Ok(());
    }
    let mut part_paths = Vec::new();
    let mut downloaded_parts = Vec::new();
    let mut total_bytes = 0_usize;
    for (position, part) in parts.iter().enumerate() {
        let part_url = part
            .get("url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "merged native part has no URL".to_owned())?;
        let part_path = merge_part_path(&output, merged_ext, part, position)?;
        let mut request =
            options.request_for_url(part_url, extraction_context.cookie_jar().clone());
        native_apply_info_http_headers(&mut request, &merged_info)?;
        if let Some(extra_param) = format_extra_param(part)? {
            request.extensions_mut().insert(
                "extra_param_to_segment_url".to_owned(),
                serde_json::json!(extra_param),
            );
        }
        let part_ext = part.get("ext").and_then(serde_json::Value::as_str);
        let part_protocol = native_format_protocol(info, part, part_url);
        let result = native_dispatch_download(
            downloader,
            &request,
            part_url,
            part_ext,
            part_protocol.as_deref(),
            &part_path,
            download_options,
        )?;
        let mut final_path = part_path.clone();
        if let Some(actual) = result.path.as_ref() {
            if actual != &final_path {
                std::fs::copy(actual, &final_path).map_err(|error| error.to_string())?;
                final_path = actual.clone();
            }
        }
        total_bytes += result.bytes;
        let mut downloaded = part.clone();
        if let Some(object) = downloaded.as_object_mut() {
            object.insert(
                "filepath".to_owned(),
                serde_json::json!(final_path.to_string_lossy()),
            );
        }
        downloaded_parts.push(downloaded);
        part_paths.push(final_path);
    }
    let mut merge_info = merged_info.clone();
    merge_info.insert("filepath", serde_json::json!(output.to_string_lossy()));
    merge_info.insert(
        "requested_formats",
        serde_json::Value::Array(downloaded_parts),
    );
    merge_info.insert(
        "__files_to_merge",
        serde_json::Value::Array(
            part_paths
                .iter()
                .map(|path| serde_json::json!(path.to_string_lossy()))
                .collect(),
        ),
    );
    let merge_options = native_postprocess_options(options, download_options.simulate);
    let merge_result = FfmpegMerger
        .run(&merge_info, &merge_options)
        .map_err(|error| error.to_string())?;
    for path in &merge_result.files_to_delete {
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    let postprocessed =
        if options.extractaudio || options.remuxvideo.is_some() || options.recodevideo.is_some() {
            let mut post_info = merge_info.clone();
            post_info.insert("filepath", serde_json::json!(output.to_string_lossy()));
            Some(run_native_postprocessor(
                &post_info,
                options,
                merge_result.simulated,
            )?)
        } else {
            None
        };
    if !merge_result.simulated && !options.skip_download {
        archive
            .record_info(&merged_info, fallback_extractor)
            .map_err(|error| error.to_string())?;
    }
    if options.dumpjson || options.dump_single_json {
        let mut output_json = serde_json::json!({
            "url": parts
                .iter()
                .filter_map(|part| part.get("url").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("\n"),
            "bytes": total_bytes,
            "path": output.to_string_lossy(),
            "simulated": merge_result.simulated,
        });
        if let Some(postprocessed) = postprocessed {
            output_json["postprocess"] = postprocess_result_json(&postprocessed);
        }
        println!(
            "{}",
            serde_json::to_string(&output_json).map_err(|error| error.to_string())?
        );
    } else if merge_result.simulated {
        println!(
            "[download] simulated {} bytes from {}",
            total_bytes,
            output.display()
        );
    } else {
        println!("[download] {} bytes -> {}", total_bytes, output.display());
    }
    Ok(())
}

/// Name one merge part `f<format_id>` file next to the merged output, like
/// `prepend_extension(correct_ext(temp, ext), 'f<id>', ext)`.
fn merge_part_path(
    output: &std::path::Path,
    merged_ext: Option<&str>,
    part: &serde_json::Value,
    position: usize,
) -> Result<PathBuf, String> {
    let merged_ext =
        merged_ext.ok_or_else(|| "merged native format has no extension".to_owned())?;
    let name = output.to_string_lossy();
    let stem = match name.rsplit_once('.') {
        Some((stem, ext)) if ext == merged_ext => stem.to_owned(),
        _ => name.into_owned(),
    };
    let part_id = part
        .get("format_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| position.to_string());
    Ok(PathBuf::from(format!("{stem}.f{part_id}.{merged_ext}")))
}

fn native_apply_info_http_headers(request: &mut Request, info: &InfoDict) -> Result<(), String> {
    let Some(headers) = info.get("http_headers") else {
        return Ok(());
    };
    let Some(headers) = headers.as_object() else {
        return Err("TODO: native downloader requires http_headers to be an object".to_owned());
    };
    for (name, value) in headers {
        let Some(value) = value.as_str() else {
            return Err(format!(
                "TODO: native downloader requires string http_headers values for {name}"
            ));
        };
        if !request.headers().contains(name) {
            request.headers_mut().set(name, value);
        }
    }
    Ok(())
}

fn native_protocol_todo(
    url: &str,
    extension: Option<&str>,
    protocol: Option<&str>,
) -> Option<String> {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .unwrap_or_default();
    let lower_url = url.to_ascii_lowercase();
    let inferred_extension = yt_dlp_core::determine_ext(Some(url), "");
    let extension = extension
        .map(str::to_ascii_lowercase)
        .or_else(|| (!inferred_extension.is_empty()).then(|| inferred_extension.to_ascii_lowercase()));

    let reason = match scheme.as_str() {
        "rtmp" | "rtmps" | "rtmpe" | "rtmpt" | "rtmpte" => Some("RTMP"),
        "mms" | "mmsh" | "rtsp" => Some("legacy streaming transport"),
        "ftp" | "sftp" => Some("non-HTTP transport"),
        _ if lower_url.contains(".ism/manifest") || lower_url.contains(".isml/manifest") => {
            Some("Microsoft Smooth Streaming")
        }
        _ if protocol.is_some_and(|protocol| {
            matches!(
                protocol.to_ascii_lowercase().as_str(),
                "f4m" | "hds" | "ism" | "isml" | "mss" | "rtmp" | "rtmps" | "smil"
            )
        }) => protocol.and_then(|protocol| match protocol.to_ascii_lowercase().as_str() {
            "f4m" | "hds" => Some("Adobe HDS/F4M"),
            "ism" | "isml" | "mss" => Some("Microsoft Smooth Streaming"),
            "rtmp" | "rtmps" => Some("RTMP"),
            "smil" => Some("SMIL playlist"),
            _ => None,
        }),
        _ => match extension.as_deref() {
            Some("f4m" | "f4f") => Some("Adobe HDS/F4M"),
            Some("ism" | "isml" | "mss") => Some("Microsoft Smooth Streaming"),
            _ if !scheme.is_empty() && !matches!(scheme.as_str(), "http" | "https") => {
                Some("unsupported URL scheme")
            }
            _ => None,
        },
    }?;

    Some(format!(
        "TODO: native downloader does not implement {reason} media for {url}"
    ))
}

fn native_url_ends_with(url: &str, suffix: &str) -> bool {
    url.split('?')
        .next()
        .map(|url| url.to_ascii_lowercase().ends_with(suffix))
        .unwrap_or(false)
}

fn native_hls_protocol(protocol: &str) -> bool {
    matches!(protocol.to_ascii_lowercase().as_str(), "hls" | "m3u8" | "m3u8_native")
}

fn native_dash_protocol(protocol: &str) -> bool {
    matches!(
        protocol.to_ascii_lowercase().as_str(),
        "dash" | "mpd" | "http_dash_segments"
    )
}
