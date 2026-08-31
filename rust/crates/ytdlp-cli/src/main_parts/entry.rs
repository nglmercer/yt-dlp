fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("--help") | Some("-h") => print_help(),
        Some("--version") => println!("{MIGRATION_VERSION}"),
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
        Some("--native-postprocess") => {
            if let Err(error) = native_postprocess_argument(&args[1..]) {
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
            if let Err(error) = native_download_argument(&args) {
                eprintln!("yt-dlp-rs: {error}");
                std::process::exit(2);
            }
        }
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;

    fn sample_info() -> InfoDict {
        let mut info = InfoDict::new();
        info.insert(
            "formats",
            serde_json::json!([
                {"format_id": "ogg", "ext": "ogg", "vcodec": "none", "url": "https://media.test/a.ogg"},
                {"format_id": "mp3", "ext": "mp3", "vcodec": "none", "url": "https://media.test/a.mp3"},
                {"format_id": "video", "ext": "mp4", "url": "https://media.test/a.mp4"}
            ]),
        );
        info
    }

    #[test]
    fn native_format_selection_supports_exact_audio_and_video_aliases() {
        let info = sample_info();
        assert_eq!(
            select_download_format(&info, Some("mp3")).unwrap().0,
            "https://media.test/a.mp3"
        );
        assert_eq!(
            select_download_format(&info, Some("bv")).unwrap().0,
            "https://media.test/a.mp4"
        );
        assert_eq!(
            select_download_format(&info, Some("ba")).unwrap().0,
            "https://media.test/a.ogg"
        );
    }

    #[test]
    fn complex_native_format_selection_is_explicitly_todo() {
        let error =
            select_download_format(&sample_info(), Some("bestvideo+bestaudio")).unwrap_err();
        assert!(error.starts_with("TODO:"));
    }

    #[test]
    fn native_downloader_marks_unimplemented_protocols_as_todo() {
        let rtmp_error =
            native_protocol_todo("rtmp://media.test/live/stream", Some("flv"), None);
        assert!(rtmp_error
            .as_deref()
            .is_some_and(|error| error.starts_with("TODO:")));

        let hds_error = native_protocol_todo("https://media.test/video.f4m", None, None);
        assert!(hds_error
            .as_deref()
            .is_some_and(|error| error.contains("Adobe HDS/F4M")));

        let smooth_error =
            native_protocol_todo("https://media.test/video.ism/Manifest", None, None);
        assert!(smooth_error
            .as_deref()
            .is_some_and(|error| error.contains("Microsoft Smooth Streaming")));

        let declared_hds =
            native_protocol_todo("https://media.test/manifest", Some("mp4"), Some("hds"));
        assert!(declared_hds
            .as_deref()
            .is_some_and(|error| error.contains("Adobe HDS/F4M")));

        let smil_error =
            native_protocol_todo("https://media.test/video.smil", Some("mp4"), Some("smil"));
        assert!(smil_error
            .as_deref()
            .is_some_and(|error| error.contains("SMIL playlist")));

        assert!(native_hls_protocol("m3u8_native"));
        assert!(native_dash_protocol("http_dash_segments"));
        assert!(
            native_protocol_todo("https://media.test/video.mp4", Some("mp4"), None).is_none()
        );
        assert!(native_protocol_todo("https://media.test/video.m3u8", None, None).is_none());
    }

    #[test]
    fn native_input_urls_combines_command_line_and_batch_file_entries() {
        let path = std::env::temp_dir().join(format!(
            "yt-dlp-rs-batch-{}-{}.txt",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(
            &path,
            "\n# ignored\nhttps://example.test/one\n  https://example.test/two  \n",
        )
        .unwrap();
        let mut options = cli::CliOptions::default();
        options.urls.push("https://example.test/zero".to_owned());
        options.batchfile = Some(path.to_string_lossy().into_owned());

        let urls = native_input_urls(&options).unwrap();
        assert_eq!(
            urls,
            vec![
                "https://example.test/zero",
                "https://example.test/one",
                "https://example.test/two"
            ]
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_playlist_indices_supports_ranges_and_last_entry() {
        assert_eq!(
            native_playlist_indices(Some("1,3-4,-1"), 5).unwrap(),
            vec![0, 2, 3, 4]
        );
        assert_eq!(
            native_playlist_indices(Some("3,1,1,-2:-1"), 5).unwrap(),
            vec![2, 0, 0, 3, 4]
        );
        assert_eq!(
            native_playlist_indices(Some("5:1:-2"), 5).unwrap(),
            vec![4, 2, 0]
        );
        assert_eq!(
            native_playlist_indices(Some(":3"), 5).unwrap(),
            vec![0, 1, 2]
        );
        assert!(native_playlist_indices(Some("0"), 5).unwrap().is_empty());
        assert!(native_playlist_indices(Some("4-2"), 5).unwrap().is_empty());
        assert!(native_playlist_indices(Some("1:3:0"), 5).is_err());
    }
}
