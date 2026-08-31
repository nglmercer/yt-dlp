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
