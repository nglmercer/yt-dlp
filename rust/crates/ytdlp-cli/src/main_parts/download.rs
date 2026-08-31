fn native_download_argument(args: &[String]) -> Result<(), String> {
    let result = parse_configured_args(args).map_err(|error| error.to_string())?;
    let ParseResult::Options(options) = result else {
        return parse_options_result(result);
    };
    let urls = native_input_urls(&options)?;
    let registry = ExtractorRegistry::generated().map_err(|error| error.to_string())?;
    let extraction_context = ExtractionContext::native();
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
        ExtractorResult::Single(info) => native_download_info(
            options,
            &info,
            extraction_context,
            archive,
            Some(&extractor_key),
        ),
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
        ExtractorResult::Single(info) => info,
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
    let declared_ext = selected_ext
        .as_deref()
        .or_else(|| info.get("ext").and_then(serde_json::Value::as_str));
    let declared_protocol = info
        .get("protocol")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            format_records(info).into_iter().find_map(|format| {
                (format.get("url").and_then(serde_json::Value::as_str)
                    == Some(download_url.as_str()))
                .then(|| format.get("protocol").and_then(serde_json::Value::as_str))
                .flatten()
            })
        });
    if let Some(todo) = native_protocol_todo(&download_url, declared_ext, declared_protocol) {
        return Err(todo);
    }
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
    let result = match declared_ext {
        Some("m3u8") => downloader.download_hls(&request, Some(&output), &download_options),
        Some("mpd") => downloader.download_dash(&request, Some(&output), &download_options),
        _ if declared_protocol.is_some_and(native_hls_protocol) => {
            downloader.download_hls(&request, Some(&output), &download_options)
        }
        _ if declared_protocol.is_some_and(native_dash_protocol) => {
            downloader.download_dash(&request, Some(&output), &download_options)
        }
        _ if native_url_ends_with(&download_url, ".m3u8") => {
            downloader.download_hls(&request, Some(&output), &download_options)
        }
        _ if native_url_ends_with(&download_url, ".mpd") => {
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
    if let Some(info_path) = info_path {
        eprintln!("[info] {}", info_path.display());
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
