/// Parse the migration's typed CLI subset.
///
/// The parser deliberately reports unknown options instead of silently
/// accepting them. The supported set is expanded in lockstep with the source
/// option schema and differential fixtures.
pub fn parse_args(args: &[String]) -> Result<ParseResult, CliError> {
    let args = expand_aliases(args)?;
    parse_args_inner(&args)
}

fn parse_args_inner(args: &[String]) -> Result<ParseResult, CliError> {
    let mut options = CliOptions::default();
    let mut index = 0;
    let mut parse_options = true;

    while index < args.len() {
        let argument = &args[index];
        if parse_options && argument == "--" {
            parse_options = false;
            index += 1;
            continue;
        }
        if !parse_options || !argument.starts_with('-') || argument == "-" {
            options.urls.push(argument.clone());
            index += 1;
            continue;
        }

        if argument == "-h" || argument == "--help" {
            return Ok(ParseResult::Help);
        }
        if argument == "--version" {
            return Ok(ParseResult::Version);
        }

        if argument.starts_with("--") {
            let (option, inline_value) = split_long_option(argument);
            match option {
                "--proxy" => {
                    options.proxy = Some(option_value(args, &mut index, option, inline_value)?)
                }
                "--socket-timeout" => {
                    options.socket_timeout = Some(parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--no-check-certificates" => options.no_check_certificate = true,
                "--js-runtimes" => {
                    options
                        .js_runtimes
                        .push(option_value(args, &mut index, option, inline_value)?)
                }
                "--no-js-runtimes" => options.js_runtimes.clear(),
                "--remote-components" => options.remote_components.push(option_value(
                    args,
                    &mut index,
                    option,
                    inline_value,
                )?),
                "--no-remote-components" => options.remote_components.clear(),
                "--ignore-config" | "--no-config" => options.ignoreconfig = Some(true),
                "--download-archive" => {
                    options.download_archive =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--no-download-archive" => options.download_archive = None,
                "--cookies" => {
                    options.cookiefile =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--no-cookies" => options.cookiefile = None,
                "--no-config-locations" => options.config_locations = None,
                "--config-locations" => {
                    options
                        .config_locations
                        .get_or_insert_with(Vec::new)
                        .push(option_value(args, &mut index, option, inline_value)?);
                }
                "--user-agent" => {
                    options.user_agent =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--referer" => {
                    options.referer = Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--add-headers" => {
                    let value = option_value(args, &mut index, option, inline_value)?;
                    let (name, value) = value
                        .split_once(':')
                        .ok_or_else(|| CliError::new("--add-headers must be FIELD:VALUE"))?;
                    options
                        .headers
                        .insert(name.to_ascii_lowercase(), value.to_owned());
                }
                "--quiet" => options.quiet = Some(true),
                "--no-quiet" => options.quiet = Some(false),
                "--verbose" => options.verbose = true,
                "--no-warnings" => options.no_warnings = true,
                "--simulate" => options.simulate = Some(true),
                "--no-simulate" => options.simulate = Some(false),
                "--skip-download" | "--no-download" => options.skip_download = true,
                "--get-url" => options.geturl = Some(true),
                "--get-title" => options.gettitle = Some(true),
                "--get-id" => options.getid = Some(true),
                "--get-thumbnail" => options.getthumbnail = Some(true),
                "--get-duration" => options.getduration = Some(true),
                "--write-info-json" => options.writeinfojson = Some(true),
                "--no-write-info-json" => options.writeinfojson = Some(false),
                "--format" => {
                    options.format = Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--all-formats" => options.format = Some("all".to_owned()),
                "--format-sort" => add_csv(
                    &mut options.format_sort,
                    &option_value(args, &mut index, option, inline_value)?,
                    true,
                ),
                "--format-sort-reset" => options.format_sort.clear(),
                "--extract-audio" => options.extractaudio = true,
                "--audio-format" => {
                    options.audioformat =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--audio-quality" => {
                    options.audioquality =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--merge-output-format" => {
                    options.merge_output_format =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--remux-video" => {
                    options.remuxvideo =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--recode-video" => {
                    options.recodevideo =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--postprocessor-args" | "--ppa" => {
                    let value = option_value(args, &mut index, option, inline_value)?;
                    add_postprocessor_args(&mut options.postprocessor_args, &value)?;
                }
                "--keep-video" => options.keepvideo = true,
                "--no-keep-video" => options.keepvideo = false,
                "--post-overwrites" => options.nopostoverwrites = false,
                "--no-post-overwrites" => options.nopostoverwrites = true,
                "--ffmpeg-location" => {
                    options.ffmpeg_location =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--sleep-subtitles" => {
                    options.sleep_interval_subtitles = parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?;
                }
                "--sleep-requests" => {
                    options.sleep_interval_requests = Some(parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--sleep-interval" | "--min-sleep-interval" => {
                    options.sleep_interval = Some(parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--max-sleep-interval" => {
                    options.max_sleep_interval = Some(parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--output" => add_prefixed_value(
                    &mut options.outtmpl,
                    &option_value(args, &mut index, option, inline_value)?,
                    &[
                        "default",
                        "chapter",
                        "thumbnail",
                        "pl_thumbnail",
                        "pl_video",
                        "pl_audio",
                        "pl_infojson",
                        "subtitle",
                    ],
                    "default",
                ),
                "--no-overwrites" => options.overwrites = Some(false),
                "--force-overwrites" | "--yes-overwrites" => options.overwrites = Some(true),
                "--no-force-overwrites" => options.overwrites = None,
                "--continue" => options.continue_dl = true,
                "--no-continue" => options.continue_dl = false,
                "--no-playlist" => options.noplaylist = true,
                "--yes-playlist" => options.noplaylist = false,
                "--list-formats" => options.listformats = Some(true),
                "--batch-file" => {
                    options.batchfile = Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--playlist-items" => {
                    options.playlist_items =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--age-limit" => {
                    options.age_limit = Some(parse_i64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--retries" => {
                    options.retries = serde_json::Value::String(option_value(
                        args,
                        &mut index,
                        option,
                        inline_value,
                    )?);
                }
                "--concurrent-fragments" => {
                    options.concurrent_fragments = parse_i64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?;
                }
                _ => return Err(CliError::new(format!("unknown option {option}"))),
            }
            index += 1;
            continue;
        }

        let short = argument.as_str();
        if short.len() > 2 && !matches!(&short[..2], "-f" | "-S" | "-o" | "-a" | "-P" | "-u" | "-p")
        {
            for flag in short[1..].chars() {
                match flag {
                    'q' => options.quiet = Some(true),
                    'v' => options.verbose = true,
                    's' => options.simulate = Some(true),
                    'j' => options.dumpjson = true,
                    'J' => options.dump_single_json = true,
                    'F' => options.listformats = Some(true),
                    'g' => options.geturl = Some(true),
                    'e' => options.gettitle = Some(true),
                    'x' => options.extractaudio = true,
                    'k' => options.keepvideo = true,
                    'i' | 'n' => {}
                    'c' => options.continue_dl = true,
                    'w' => options.overwrites = Some(false),
                    _ => return Err(CliError::new(format!("unknown option -{flag}"))),
                }
            }
            index += 1;
            continue;
        }
        let (flag, suffix) = if short.len() > 2 {
            short.split_at(2)
        } else {
            (short, "")
        };
        let value_option = matches!(flag, "-f" | "-S" | "-o" | "-a" | "-P" | "-u" | "-p");
        if value_option {
            let value = if suffix.is_empty() {
                next_value(args, &mut index, flag)?
            } else {
                suffix.to_owned()
            };
            match flag {
                "-f" => options.format = Some(value),
                "-S" => add_csv(&mut options.format_sort, &value, true),
                "-o" => add_prefixed_value(
                    &mut options.outtmpl,
                    &value,
                    &[
                        "default",
                        "chapter",
                        "thumbnail",
                        "pl_thumbnail",
                        "pl_video",
                        "pl_audio",
                        "pl_infojson",
                        "subtitle",
                    ],
                    "default",
                ),
                "-a" => options.batchfile = Some(value),
                "-P" => add_prefixed_value(&mut options.outtmpl, &value, &["home", "temp"], "home"),
                "-u" | "-p" => {
                    return Err(CliError::new(format!("{flag} is not active in Rust yet")));
                }
                _ => unreachable!(),
            }
            index += 1;
            continue;
        }

        match short {
            "-q" => options.quiet = Some(true),
            "-v" => options.verbose = true,
            "-s" => options.simulate = Some(true),
            "-j" => options.dumpjson = true,
            "-J" => options.dump_single_json = true,
            "-F" => options.listformats = Some(true),
            "-g" => options.geturl = Some(true),
            "-e" => options.gettitle = Some(true),
            "-x" => options.extractaudio = true,
            "-k" => options.keepvideo = true,
            "-i" => {}
            "-n" => {}
            "-c" => options.continue_dl = true,
            "-w" => options.overwrites = Some(false),
            _ => return Err(CliError::new(format!("unknown option {short}"))),
        }
        index += 1;
    }

    Ok(ParseResult::Options(options))
}
