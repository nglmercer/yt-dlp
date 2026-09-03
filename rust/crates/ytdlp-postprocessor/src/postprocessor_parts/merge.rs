/// Native port of `FFmpegMergerPP` (`yt_dlp/postprocessor/ffmpeg.py`).
///
/// Merges previously downloaded `__files_to_merge` parts into the final
/// `filepath` with stream copy, using one `-map` per audio/video stream in
/// `requested_formats`.

/// Insert an extra extension before the real one: `movie.mp4` plus `temp`
/// becomes `movie.temp.mp4`, mirroring `prepend_extension`.
fn prepend_merge_extension(filename: &PathBuf, extension: &str) -> PathBuf {
    let name = filename.to_string_lossy();
    match name.rsplit_once('.') {
        Some((stem, real_ext)) if !stem.is_empty() => {
            PathBuf::from(format!("{stem}.{extension}.{real_ext}"))
        }
        _ => PathBuf::from(format!("{name}.{extension}")),
    }
}

pub struct FfmpegMerger;

impl PostProcessor for FfmpegMerger {
    fn key(&self) -> &str {
        "FFmpegMerger"
    }

    fn run(
        &self,
        info: &InfoDict,
        options: &PostProcessOptions,
    ) -> Result<PostProcessResult, PostProcessError> {
        let missing = |field: &str| PostProcessError::MissingField(field.to_owned());
        let output = info_path(info)?;
        let parts = info
            .get("__files_to_merge")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| missing("__files_to_merge"))?
            .iter()
            .map(|part| {
                part.as_str()
                    .map(PathBuf::from)
                    .ok_or_else(|| missing("__files_to_merge"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let requested = info
            .get("requested_formats")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| missing("requested_formats"))?;
        let temp_output = prepend_merge_extension(&output, "temp");
        ensure_output_available(&temp_output, options.overwrite)?;
        let probe = MediaProbe::new(options.ffmpeg_location.as_deref());
        let mut operation_args = vec!["-c".to_owned(), "copy".to_owned()];
        let mut audio_streams = 0;
        for (index, format) in requested.iter().enumerate() {
            if format.get("acodec").and_then(serde_json::Value::as_str) != Some("none") {
                operation_args.push("-map".to_owned());
                operation_args.push(format!("{index}:a:0"));
                // Mirrors `FFmpegMergerPP`: HLS AAC parts are ADTS-packed, so
                // they need the `aac_adtstoasc` bitstream filter. A missing
                // protocol counts as non-HLS (the local fixtures omit it);
                // upstream raises KeyError there instead.
                let is_hls = format
                    .get("protocol")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|protocol| protocol.starts_with("m3u8"));
                if is_hls {
                    let part_path = format
                        .get("filepath")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| missing("filepath"))?;
                    if probe.audio_codec(Path::new(part_path))? == Some("aac".to_owned()) {
                        operation_args.push(format!("-bsf:a:{audio_streams}"));
                        operation_args.push("aac_adtstoasc".to_owned());
                    }
                }
                audio_streams += 1;
            }
            if format.get("vcodec").and_then(serde_json::Value::as_str) != Some("none") {
                operation_args.push("-map".to_owned());
                operation_args.push(format!("{index}:v:0"));
            }
        }
        let runner = FfmpegRunner::new(options.ffmpeg_location.as_deref());
        // Mirror `real_run_ffmpeg`: `-y`, `-loglevel repeat+info` for
        // ffmpeg, one `-i` per part, then the operation arguments.
        let mut args = vec![if options.overwrite { "-y" } else { "-n" }.to_owned()];
        if runner
            .executable()
            .file_name()
            .and_then(|name| name.to_str())
            == Some("ffmpeg")
        {
            args.push("-loglevel".to_owned());
            args.push("repeat+info".to_owned());
        }
        for part in &parts {
            args.push("-i".to_owned());
            args.push(ffmpeg_file_argument(part));
        }
        args.extend(operation_args);
        args.extend(extra_args(options, self.key(), "ffmpeg_o1"));
        args.push("-movflags".to_owned());
        args.push("+faststart".to_owned());
        args.push(ffmpeg_file_argument(&temp_output));
        let command = FfmpegCommand {
            program: runner.executable().to_path_buf(),
            args: args.into_iter().map(std::ffi::OsString::from).collect(),
        };
        let argv = runner.run(&command, options.simulate)?;
        if !options.simulate {
            ensure_output_created(&temp_output, options.simulate)?;
            std::fs::rename(&temp_output, &output)?;
        }
        Ok(PostProcessResult {
            files_to_delete: (!options.keep_video).then_some(parts).unwrap_or_default(),
            info: info.clone(),
            command: argv,
            simulated: options.simulate,
        })
    }
}
