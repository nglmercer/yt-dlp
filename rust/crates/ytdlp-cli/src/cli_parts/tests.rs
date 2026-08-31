#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_common_options_and_preserves_last_value() {
        let parsed = parse_args(&args(&[
            "--proxy=http://proxy.example",
            "--socket-timeout",
            "4.5",
            "--no-check-certificates",
            "--add-headers",
            "X-Test: one",
            "--add-headers",
            "Cookie:a=b",
            "-q",
            "-f",
            "bv*",
            "-o",
            "%(id)s.%(ext)s",
            "https://example.com",
        ]))
        .unwrap();
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };

        assert_eq!(options.proxy.as_deref(), Some("http://proxy.example"));
        assert_eq!(options.socket_timeout, Some(4.5));
        assert!(options.no_check_certificate);
        assert_eq!(options.headers["x-test"], " one");
        assert_eq!(options.headers["cookie"], "a=b");
        assert_eq!(options.user_agent, None);
        assert_eq!(options.referer, None);
        assert_eq!(options.quiet, Some(true));
        assert_eq!(options.format.as_deref(), Some("bv*"));
        assert_eq!(options.outtmpl["default"], "%(id)s.%(ext)s");
        assert_eq!(options.urls, ["https://example.com"]);
    }

    #[test]
    fn parses_aliases_and_option_terminator() {
        let parsed = parse_args(&args(&[
            "--no-playlist",
            "--yes-playlist",
            "--skip-download",
            "--no-simulate",
            "-v",
            "--",
            "-not-an-option",
        ]))
        .unwrap();
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };
        assert!(!options.noplaylist);
        assert!(options.skip_download);
        assert_eq!(options.simulate, Some(false));
        assert!(options.verbose);
        assert_eq!(options.urls, ["-not-an-option"]);
    }

    #[test]
    fn rejects_unknown_options_and_bad_values() {
        assert!(parse_args(&args(&["--not-real"])).is_err());
        assert!(parse_args(&args(&["--socket-timeout", "slow"])).is_err());
        assert!(parse_args(&args(&["--add-headers", "missing-value"])).is_err());
    }

    #[test]
    fn expands_dynamic_and_preset_aliases() {
        let parsed = parse_args(&args(&[
            "--alias",
            "quick,-Q",
            "--format {0} --quiet",
            "--quick",
            "bestvideo",
            "video",
        ]))
        .unwrap();
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };
        assert_eq!(options.format.as_deref(), Some("bestvideo"));
        assert_eq!(options.quiet, Some(true));
        assert_eq!(options.urls, ["video"]);

        let ParseResult::Options(options) = parse_args(&args(&["-t", "mp3", "video"])).unwrap()
        else {
            panic!("expected options")
        };
        assert!(options.extractaudio);
        assert_eq!(options.audioformat.as_deref(), Some("mp3"));
    }

    #[test]
    fn config_files_are_tokenized_and_overridden_by_command_line() {
        let path = std::env::temp_dir().join(format!(
            "yt-dlp-rs-config-{}-{}.conf",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(
            &path,
            "# comment\n--quiet -o 'config.%(ext)s'\n--alias cfg --format {0}\n",
        )
        .unwrap();

        let parsed = parse_args_with_config_files(
            &args(&["--cfg", "best", "-o", "command.%(ext)s", "video"]),
            std::slice::from_ref(&path),
        )
        .unwrap();
        let _ = std::fs::remove_file(&path);
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };
        assert_eq!(options.quiet, Some(true));
        assert_eq!(options.format.as_deref(), Some("best"));
        assert_eq!(options.outtmpl["default"], "command.%(ext)s");
    }

    #[test]
    fn request_adapter_carries_network_options_into_native_request() {
        let parsed = parse_args(&args(&[
            "--proxy",
            "http://proxy.example:8080",
            "--socket-timeout",
            "3.5",
            "--no-check-certificates",
            "--add-headers",
            "X-Trace: enabled",
            "--user-agent",
            "Rust test agent",
            "--referer",
            "https://referrer.example/",
            "https://example.com/video",
        ]))
        .unwrap();
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };

        let request = options.request_for_url(
            &options.urls[0],
            yt_dlp_networking::CookieJar::new().shared(),
        );
        assert_eq!(request.headers().get("X-Trace"), Some("enabled"));
        assert_eq!(request.headers().get("User-Agent"), Some("Rust test agent"));
        assert_eq!(
            request.headers().get("Referer"),
            Some("https://referrer.example/")
        );
        assert_eq!(
            request.proxies().get("all").and_then(Option::as_deref),
            Some("http://proxy.example:8080")
        );
        assert_eq!(
            request
                .extensions()
                .get("timeout")
                .and_then(serde_json::Value::as_f64),
            Some(3.5)
        );
        assert_eq!(
            request.extensions().get("verify"),
            Some(&serde_json::json!(false))
        );
    }
}
