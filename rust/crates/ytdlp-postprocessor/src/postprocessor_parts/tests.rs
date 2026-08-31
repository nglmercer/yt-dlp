#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_safe_remux_arguments_without_a_shell() {
        let runner = FfmpegRunner::new(Some(Path::new("/opt/tools/ffmpeg")));
        let mut options = PostProcessOptions::default();
        options
            .extra_args
            .insert("default-compat".to_owned(), vec!["-hide_banner".to_owned()]);
        let command = runner.build_command(
            Path::new("video;$(touch pwned).mp4"),
            Path::new("video.mkv"),
            &["-c".to_owned(), "copy".to_owned()],
            &options,
            "FFmpegVideoRemuxer",
        );
        assert_eq!(command.program, PathBuf::from("/opt/tools/ffmpeg"));
        assert!(
            command
                .args
                .iter()
                .any(|arg| arg == "file:video;$(touch pwned).mp4")
        );
        assert!(command.args.iter().any(|arg| arg == "-hide_banner"));
    }

    #[test]
    fn dry_run_updates_info_and_preserves_input_by_default_when_requested() {
        let mut info = InfoDict::new();
        info.insert("filepath", json!("video.webm"));
        info.insert("ext", json!("webm"));
        let mut options = PostProcessOptions {
            simulate: true,
            keep_video: true,
            ..PostProcessOptions::default()
        };
        options.ffmpeg_location = Some(PathBuf::from("ffmpeg"));
        let result = FfmpegRemuxer::new("mkv")
            .unwrap()
            .run(&info, &options)
            .unwrap();
        assert_eq!(result.info.get_str("filepath"), Some("video.mkv"));
        assert_eq!(result.info.get_str("ext"), Some("mkv"));
        assert!(result.files_to_delete.is_empty());
        assert!(result.simulated);
        assert!(result.command.is_some());
    }

    #[test]
    fn rejects_unsafe_extensions_and_missing_filepaths() {
        assert!(FfmpegRemuxer::new("../mkv").is_err());
        assert!(FfmpegExtractAudio::new("", None).is_err());
        assert!(matches!(
            FfmpegRemuxer::new("mkv")
                .unwrap()
                .run(&InfoDict::new(), &PostProcessOptions::default()),
            Err(PostProcessError::MissingField(_))
        ));
    }

    #[test]
    fn dry_run_converter_selects_target_codecs() {
        let mut info = InfoDict::new();
        info.insert("filepath", json!("video.mp4"));
        info.insert("ext", json!("mp4"));
        let result = FfmpegVideoConvertor::new("webm")
            .unwrap()
            .run(
                &info,
                &PostProcessOptions {
                    simulate: true,
                    ..PostProcessOptions::default()
                },
            )
            .unwrap();
        let command = result.command.unwrap();
        assert!(command.iter().any(|arg| arg == "libvpx-vp9"));
        assert!(command.iter().any(|arg| arg == "libopus"));
        assert_eq!(result.info.get_str("ext"), Some("webm"));
    }
}
