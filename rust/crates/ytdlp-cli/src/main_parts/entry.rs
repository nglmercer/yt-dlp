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
    use yt_dlp_extractor::{ExtractorDescriptor, GenericExtractor};

    fn sample_info() -> InfoDict {
        let mut info = InfoDict::new();
        info.insert(
            "formats",
            serde_json::json!([
                {"format_id": "ogg", "ext": "ogg", "vcodec": "none", "acodec": "vorbis", "url": "https://media.test/a.ogg"},
                {"format_id": "mp3", "ext": "mp3", "vcodec": "none", "acodec": "mp3", "url": "https://media.test/a.mp3"},
                {"format_id": "video", "ext": "mp4", "vcodec": "vp9", "acodec": "none", "url": "https://media.test/a.mp4"}
            ]),
        );
        info
    }

    /// A YouTube-shaped adaptive fixture in scrambled extractor order. The
    /// expectations below were verified against the Python oracle.
    fn mixed_format_info() -> InfoDict {
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!("clip"));
        info.insert("title", serde_json::json!("Clip"));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([
                {"format_id": "18", "url": "https://cdn.example/18.mp4", "ext": "mp4", "vcodec": "avc1.42001E", "acodec": "mp4a.40.2", "height": 360, "width": 640, "tbr": 500},
                {"format_id": "137", "url": "https://cdn.example/137.mp4", "ext": "mp4", "vcodec": "avc1.640028", "acodec": "none", "height": 1080, "width": 1920, "tbr": 4000, "filesize": 100000000},
                {"format_id": "140", "url": "https://cdn.example/140.m4a", "ext": "m4a", "vcodec": "none", "acodec": "mp4a.40.2", "abr": 128, "tbr": 128},
                {"format_id": "22", "url": "https://cdn.example/22.mp4", "ext": "mp4", "vcodec": "avc1.64001F", "acodec": "mp4a.40.2", "height": 720, "width": 1280, "tbr": 2000},
                {"format_id": "247", "url": "https://cdn.example/247.webm", "ext": "webm", "vcodec": "vp9", "acodec": "none", "height": 720, "width": 1280, "tbr": 1500},
                {"format_id": "251", "url": "https://cdn.example/251.webm", "ext": "webm", "vcodec": "none", "acodec": "opus", "abr": 160, "tbr": 160}
            ]),
        );
        info
    }

    fn selected_ids(selections: Vec<NativeSelection>) -> Vec<String> {
        selections
            .into_iter()
            .map(|selection| match selection {
                NativeSelection::Single(format) => format
                    .get("format_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?")
                    .to_owned(),
                NativeSelection::Merged(merged) => format!(
                    "merge:{}:{}",
                    merged
                        .get("requested_formats")
                        .and_then(serde_json::Value::as_array)
                        .map(|parts| {
                            parts
                                .iter()
                                .filter_map(|part| {
                                    part.get("format_id").and_then(serde_json::Value::as_str)
                                })
                                .collect::<Vec<_>>()
                                .join("+")
                        })
                        .unwrap_or_default(),
                    merged
                        .get("ext")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?"),
                ),
            })
            .collect()
    }

    fn select_ids(info: &InfoDict, selector: Option<&str>) -> Vec<String> {
        selected_ids(select_native_downloads(info, selector, &cli::CliOptions::default()).unwrap())
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
    fn native_format_selection_merges_video_and_audio() {
        // Oracle: vp9/mp4 video plus vorbis/ogg audio merges into mkv.
        let selections = select_native_downloads(
            &sample_info(),
            Some("bestvideo+bestaudio"),
            &cli::CliOptions::default(),
        )
        .unwrap();
        assert_eq!(selected_ids(selections), vec!["merge:video+ogg:mkv"]);
    }

    #[test]
    fn native_format_sorting_matches_oracle_order() {
        let mut formats = mixed_format_info()
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap();
        sort_native_formats(&mut formats, &[], &[]);
        let ids = formats
            .iter()
            .filter_map(|format| format.get("format_id").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        // Oracle worst-first order.
        assert_eq!(ids, vec!["140", "251", "18", "22", "247", "137"]);
    }

    #[test]
    fn native_format_atoms_match_oracle_picks() {
        let info = mixed_format_info();
        // `best` is the best progressive format, not the video-only one.
        assert_eq!(select_ids(&info, Some("best")), vec!["22"]);
        assert_eq!(select_ids(&info, Some("worst")), vec!["18"]);
        assert_eq!(select_ids(&info, Some("best.2")), vec!["18"]);
        assert_eq!(select_ids(&info, Some("bv")), vec!["137"]);
        assert_eq!(select_ids(&info, Some("bv*")), vec!["137"]);
        assert_eq!(select_ids(&info, Some("ba")), vec!["251"]);
        // `ba*` accepts any format carrying audio, including progressive.
        assert_eq!(select_ids(&info, Some("ba*")), vec!["22"]);
        assert_eq!(select_ids(&info, Some("22")), vec!["22"]);
        assert_eq!(
            select_ids(&info, Some("all")),
            vec!["137", "247", "22", "18", "251", "140"]
        );
        // No mp3 container exists, so selection is empty.
        assert!(select_ids(&info, Some("mp3")).is_empty());
    }

    #[test]
    fn native_format_merges_match_oracle() {
        let info = mixed_format_info();
        // Oracle: incompatible mp4/avc + webm/opus parts merge into mkv.
        assert_eq!(select_ids(&info, Some("bv*+ba")), vec!["merge:137+251:mkv"]);
        assert_eq!(
            select_ids(&info, Some("bv+ba/b")),
            vec!["merge:137+251:mkv"]
        );
        // A video-only 720p ceiling plus the best audio merges into webm.
        assert_eq!(
            select_ids(&info, Some("bv[height<=720]+ba/b")),
            vec!["merge:247+251:webm"]
        );
        // Without any audio-only stream the merge is empty and `/b`
        // falls back to the best progressive format.
        let mut video_only = mixed_format_info();
        video_only.insert(
            "formats",
            serde_json::json!([
                {"format_id": "137", "url": "https://cdn.example/137.mp4", "ext": "mp4", "vcodec": "avc1.640028", "acodec": "none", "height": 1080},
                {"format_id": "22", "url": "https://cdn.example/22.mp4", "ext": "mp4", "vcodec": "avc1.64001F", "acodec": "mp4a.40.2", "height": 720}
            ]),
        );
        assert_eq!(select_ids(&video_only, Some("bv+ba/b")), vec!["22"]);
    }

    #[test]
    fn native_format_filters_match_oracle() {
        let info = mixed_format_info();
        assert_eq!(select_ids(&info, Some("[height<=720]")), vec!["22"]);
        assert_eq!(select_ids(&info, Some("bv[height<=720]")), vec!["247"]);
        assert_eq!(select_ids(&info, Some("ba[acodec=opus]")), vec!["251"]);
        assert_eq!(select_ids(&info, Some("b[ext^=web]")), Vec::<String>::new());
        assert_eq!(select_ids(&info, Some("[tbr>1000]")), vec!["22"]);
    }

    #[test]
    fn native_format_sort_keys_match_oracle() {
        let info = mixed_format_info();
        let mut options = cli::CliOptions::default();
        options.format_sort = vec!["vcodec:vp9".to_owned()];
        let mut formats = info
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap();
        sort_native_formats(&mut formats, &options.format_sort, &[]);
        let ids = formats
            .iter()
            .filter_map(|format| format.get("format_id").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["140", "251", "18", "22", "137", "247"]);

        options.format_sort = vec!["res:720".to_owned()];
        let mut formats = info
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap();
        sort_native_formats(&mut formats, &options.format_sort, &[]);
        let ids = formats
            .iter()
            .filter_map(|format| format.get("format_id").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["140", "251", "137", "18", "22", "247"]);
    }

    #[test]
    fn native_format_selector_rejects_invalid_syntax() {
        for spec in ["bv+", ",bv", "(bv", "bv[height<]"] {
            assert!(
                select_native_downloads(
                    &mixed_format_info(),
                    Some(spec),
                    &cli::CliOptions::default()
                )
                .is_err(),
                "{spec} should fail"
            );
        }
    }

    #[test]
    fn native_default_format_prefers_merge_when_ffmpeg_configured() {
        let mut options = cli::CliOptions::default();
        options.ffmpeg_location = Some("/usr/bin".to_owned());
        let selections = select_native_downloads(&mixed_format_info(), None, &options).unwrap();
        assert_eq!(selected_ids(selections), vec!["merge:137+251:mkv"]);
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

    #[test]
    fn native_playlist_url_results_resolve_and_merge_transparent_metadata() {
        let mut registry = ExtractorRegistry::new();
        registry
            .register(GenericExtractor::new(ExtractorDescriptor::new(
                "GenericIE",
                "generic",
                r"https?://cdn\.test/.*",
                true,
            )))
            .unwrap();
        let context = ExtractionContext::new(
            RequestDirector::new(),
            yt_dlp_networking::CookieJar::new().shared(),
        );
        let mut entry = InfoDict::new();
        entry.insert("_type", serde_json::json!("url_transparent"));
        entry.insert("url", serde_json::json!("https://cdn.test/episode.mp4"));
        entry.insert("id", serde_json::json!("parent-id"));
        entry.insert("title", serde_json::json!("Parent title"));

        let resolved = native_resolve_playlist_entry(&registry, &context, &entry, 0).unwrap();
        assert_eq!(resolved.get_str("id"), Some("parent-id"));
        assert_eq!(resolved.get_str("title"), Some("Parent title"));
        assert_eq!(resolved.get_str("url"), Some("https://cdn.test/episode.mp4"));
        assert_eq!(resolved.get_str("ext"), Some("mp4"));
        assert_eq!(resolved.get("direct"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn native_info_http_headers_fill_request_without_overwriting_cli_headers() {
        let mut options = cli::CliOptions::default();
        options.referer = Some("https://cli.example/override".to_owned());
        let mut request = options.request_for_url(
            "https://cdn.example/video.mp4",
            CookieJar::new().shared(),
        );
        let mut info = InfoDict::new();
        info.insert(
            "http_headers",
            serde_json::json!({
                "Referer": "https://extractor.example/page",
                "X-Extractor": "native",
            }),
        );

        native_apply_info_http_headers(&mut request, &info).unwrap();

        assert_eq!(
            request.headers().get("Referer"),
            Some("https://cli.example/override")
        );
        assert_eq!(request.headers().get("X-Extractor"), Some("native"));
    }
}
