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
    fn dry_run_merger_maps_each_stream_and_keeps_final_path() {
        let mut info = InfoDict::new();
        info.insert("filepath", json!("movie.mkv"));
        info.insert("ext", json!("mkv"));
        info.insert(
            "requested_formats",
            json!([
                {"format_id": "137", "vcodec": "avc1", "acodec": "none"},
                {"format_id": "140", "vcodec": "none", "acodec": "mp4a"}
            ]),
        );
        info.insert(
            "__files_to_merge",
            json!(["movie.f137.mp4", "movie.f140.m4a"]),
        );
        let result = FfmpegMerger
            .run(
                &info,
                &PostProcessOptions {
                    simulate: true,
                    ..PostProcessOptions::default()
                },
            )
            .unwrap();
        let command = result.command.unwrap();
        let argv = command
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        // Stream copy with one map per audio/video stream, inputs in part
        // order, and the temp output with faststart.
        assert_eq!(
            argv,
            vec![
                "ffmpeg",
                "-y",
                "-loglevel",
                "repeat+info",
                "-i",
                "file:movie.f137.mp4",
                "-i",
                "file:movie.f140.m4a",
                "-c",
                "copy",
                "-map",
                "0:v:0",
                "-map",
                "1:a:0",
                "-movflags",
                "+faststart",
                "file:movie.temp.mkv",
            ]
        );
        assert_eq!(result.info.get_str("filepath"), Some("movie.mkv"));
        assert_eq!(result.files_to_delete.len(), 2);
        assert!(result.simulated);
    }

    #[test]
    fn merger_requires_parts_and_requested_formats() {
        assert!(matches!(
            FfmpegMerger.run(&InfoDict::new(), &PostProcessOptions::default()),
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

    #[test]
    fn probe_ffprobe_scan_matches_python_line_loop() {
        // (ffprobe `-show_streams` output, expected first audio codec).
        let cases = vec![
            (
                "[STREAM]\nindex=0\ncodec_name=h264\ncodec_type=video\n[/STREAM]\n\
                 [STREAM]\nindex=1\ncodec_name=aac\ncodec_type=audio\n[/STREAM]\n",
                Some("aac"),
            ),
            (
                "[STREAM]\nindex=0\ncodec_name=mp3\ncodec_type=audio\n[/STREAM]\n",
                Some("mp3"),
            ),
            (
                "[STREAM]\nindex=0\ncodec_name=h264\ncodec_type=video\n[/STREAM]\n",
                None,
            ),
            // A name placed after its type line is missed, like upstream.
            (
                "[STREAM]\nindex=0\ncodec_type=audio\ncodec_name=aac\n[/STREAM]\n",
                None,
            ),
            ("", None),
            // No case folding: the merger compares against `aac` exactly.
            (
                "[STREAM]\nindex=0\ncodec_name=AAC\ncodec_type=audio\n[/STREAM]\n",
                Some("AAC"),
            ),
            // The first audio stream wins.
            (
                "codec_name=mp3\ncodec_type=audio\ncodec_name=aac\ncodec_type=audio\n",
                Some("mp3"),
            ),
        ];
        for (output, expected) in cases {
            assert_eq!(
                scan_ffprobe_audio_codec(output).as_deref(),
                expected,
                "output {output:?}"
            );
        }
    }

    #[test]
    fn probe_ffmpeg_scan_matches_python_fallback_regex() {
        let cases = vec![
            (
                "ffmpeg version 6.0\nStream #0:0[0x1](und): Video: h264\n\
                 Stream #0:1[0x2](eng): Audio: aac (mp4a / 0x6134706D)\n",
                Some("aac"),
            ),
            ("Stream #0:0: Video: h264\n", None),
            ("nothing here", None),
            // Uppercase hex IDs fail the optional group, like the regex.
            ("Stream #0:1[0xAB](eng): Audio: aac\n", None),
            ("Stream #0:1: Audio: mp3\n", Some("mp3")),
            ("Stream #0:1(eng): Audio: opus\n", Some("opus")),
            (
                "Stream #0:1[0x2](english): Audio: aac\nStream #1:0: Audio: mp3\n",
                Some("mp3"),
            ),
            ("Stream #0:1(eng): Audio: AAC\n", None),
        ];
        for (stderr, expected) in cases {
            assert_eq!(
                scan_ffmpeg_audio_codec(stderr).as_deref(),
                expected,
                "stderr {stderr:?}"
            );
        }
    }

    #[cfg(unix)]
    fn write_fake_executable(dir: &Path, name: &str, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    static FAKE_PROBE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    #[cfg(unix)]
    fn fake_probe_dir(name: &str) -> PathBuf {
        let unique = FAKE_PROBE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ytdlp-pp-probe-{}-{unique}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    #[test]
    fn merger_adds_adtstoasc_for_hls_aac_parts() {
        let dir = fake_probe_dir("aac");
        write_fake_executable(
            &dir,
            "ffprobe",
            "#!/bin/sh\nprintf '%s' '[STREAM]\nindex=1\ncodec_name=aac\ncodec_type=audio\n[/STREAM]\n'",
        );
        let mut info = InfoDict::new();
        info.insert("filepath", json!("movie.mkv"));
        info.insert("ext", json!("mkv"));
        info.insert(
            "requested_formats",
            json!([
                {"format_id": "ba", "vcodec": "none", "acodec": "mp4a",
                 "protocol": "m3u8_native", "filepath": "movie.fba.m4a"},
                {"format_id": "bv", "vcodec": "avc1", "acodec": "none",
                 "protocol": "https", "filepath": "movie.fbv.mp4"},
            ]),
        );
        info.insert(
            "__files_to_merge",
            json!(["movie.fba.m4a", "movie.fbv.mp4"]),
        );
        let result = FfmpegMerger
            .run(
                &info,
                &PostProcessOptions {
                    ffmpeg_location: Some(dir.clone()),
                    simulate: true,
                    ..PostProcessOptions::default()
                },
            )
            .unwrap();
        let argv = result
            .command
            .unwrap()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            &argv[argv.iter().position(|arg| arg == "-c").unwrap()..],
            &[
                "-c",
                "copy",
                "-map",
                "0:a:0",
                "-bsf:a:0",
                "aac_adtstoasc",
                "-map",
                "1:v:0",
                "-movflags",
                "+faststart",
                "file:movie.temp.mkv",
            ],
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn merger_skips_adtstoasc_for_non_aac_hls() {
        let dir = fake_probe_dir("mp3");
        write_fake_executable(
            &dir,
            "ffprobe",
            "#!/bin/sh\nprintf '%s' '[STREAM]\nindex=0\ncodec_name=mp3\ncodec_type=audio\n[/STREAM]\n'",
        );
        let mut info = InfoDict::new();
        info.insert("filepath", json!("movie.mkv"));
        info.insert("ext", json!("mkv"));
        info.insert(
            "requested_formats",
            json!([
                {"format_id": "ba", "vcodec": "none", "acodec": "mp3",
                 "protocol": "m3u8_native", "filepath": "movie.fba.mp3"},
            ]),
        );
        info.insert("__files_to_merge", json!(["movie.fba.mp3"]));
        let result = FfmpegMerger
            .run(
                &info,
                &PostProcessOptions {
                    ffmpeg_location: Some(dir.clone()),
                    simulate: true,
                    ..PostProcessOptions::default()
                },
            )
            .unwrap();
        let argv = result
            .command
            .unwrap()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(!argv.iter().any(|arg| arg == "aac_adtstoasc"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn media_probe_ffmpeg_fallback_parses_audio_codec() {
        let dir = fake_probe_dir("fallback");
        let ffmpeg = write_fake_executable(
            &dir,
            "ffmpeg",
            "#!/bin/sh\nprintf '%s' 'Stream #0:1[0x2](eng): Audio: aac (mp4a)' >&2\nexit 1",
        );
        // An explicit ffmpeg file leaves ffprobe unresolved, forcing the
        // `ffmpeg -i` fallback branch.
        let probe = MediaProbe::new(Some(ffmpeg.as_path()));
        assert_eq!(
            probe.audio_codec(Path::new("part.m4a")).unwrap().as_deref(),
            Some("aac")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn media_probe_reports_missing_executables() {
        // An explicit but missing ffmpeg file resolves neither tool.
        let missing = std::env::temp_dir().join(format!(
            "ytdlp-pp-probe-{}-missing-ffmpeg",
            std::process::id()
        ));
        let error = MediaProbe::new(Some(missing.as_path()))
            .audio_codec(Path::new("part.m4a"))
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "unsupported postprocessing: ffprobe and ffmpeg not found. Please install or provide the path using --ffmpeg-location"
        );
    }
}
