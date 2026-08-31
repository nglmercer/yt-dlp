#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn info_dict_preserves_insertion_order() {
        let mut info = InfoDict::new();
        info.insert("id", json!("example"));
        info.insert("title", json!("Example"));

        assert_eq!(
            serde_json::to_string(&info).unwrap(),
            r#"{"id":"example","title":"Example"}"#
        );
    }

    #[test]
    fn info_dict_round_trips_nested_values() {
        let mut info = InfoDict::new();
        info.insert("formats", json!([{ "format_id": "best", "height": 1080 }]));

        let encoded = serde_json::to_vec(&info).unwrap();
        let decoded: InfoDict = serde_json::from_slice(&encoded).unwrap();

        assert_eq!(decoded, info);
        assert!(decoded.get("formats").is_some());
    }

    #[test]
    fn info_dict_helpers_and_output_templates_preserve_fields() {
        let mut info = InfoDict::new();
        info.insert("id", json!("abc"));
        info.insert("ext", json!("mp4"));
        info.insert("playlist_index", json!(3));
        info.insert("duration", json!(1.25));

        assert_eq!(info.get_str("id"), Some("abc"));
        assert_eq!(info.get_i64("playlist_index"), Some(3));
        assert_eq!(info.get_f64("duration"), Some(1.25));
        assert_eq!(
            render_output_template("%(playlist_index)03d-%(id)s.%(ext)s", &info).unwrap(),
            "003-abc.mp4"
        );
        assert_eq!(
            render_output_template("%(duration).2f", &info).unwrap(),
            "1.25"
        );
        assert!(matches!(
            render_output_template("%(missing)s", &info),
            Err(CoreError {
                kind: CoreErrorKind::MissingField,
                ..
            })
        ));
    }

    #[test]
    fn format_bytes_matches_reference_cases() {
        let cases = [
            (None, "N/A"),
            (Some(-1.0), "N/A"),
            (Some(-0.0), "-0.00B"),
            (Some(0.0), "0.00B"),
            (Some(1000.0), "1000.00B"),
            (Some(1024.0), "1.00KiB"),
            (Some(1024.0_f64.powi(8)), "1.00YiB"),
            (Some(1024.0_f64.powi(9)), "1024.00YiB"),
        ];

        for (input, expected) in cases {
            assert_eq!(format_bytes(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn parse_bytes_matches_reference_cases() {
        assert_eq!(parse_bytes("0"), Some(0));
        assert_eq!(parse_bytes("1.5K"), Some(1536));
        assert_eq!(parse_bytes("1Y"), Some(1024_u128.pow(8)));
        assert_eq!(parse_bytes("1,5K"), None);
        assert_eq!(parse_bytes("1KB"), None);
        assert_eq!(parse_bytes(" 1K"), None);
    }

    #[test]
    fn parse_duration_matches_reference_examples() {
        assert_eq!(parse_duration("1"), Some(1.0));
        assert_eq!(parse_duration("1337:12"), Some(80_232.0));
        assert_eq!(parse_duration("9:12:43"), Some(33_163.0));
        assert_eq!(parse_duration("3h 11m 53s"), Some(11_513.0));
        assert_eq!(parse_duration("2.5 hours"), Some(9_000.0));
        assert_eq!(parse_duration("PT1H0.040S"), Some(3_600.04));
        assert_eq!(parse_duration("01:02:03:050"), Some(3_723.05));
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn parse_iso8601_matches_utc_and_offset_examples() {
        assert_eq!(parse_iso8601("2015-04-08T00:00:00Z"), Some(1_428_451_200));
        assert_eq!(
            parse_iso8601("2015-04-08T02:00:00+02:00"),
            Some(1_428_451_200)
        );
        assert_eq!(
            parse_iso8601("2015-04-08T00:00:00-0500"),
            Some(1_428_469_200)
        );
        assert_eq!(parse_iso8601("2015-02-29T00:00:00Z"), None);
    }

    #[test]
    fn core_url_and_scalar_utilities_match_reference_examples() {
        assert_eq!(
            determine_ext(Some("https://example.test/video.mp4?download=1"), "unknown"),
            "mp4"
        );
        assert_eq!(
            determine_ext(Some("https://example.test/manifest.m3u8/"), "unknown"),
            "m3u8"
        );
        assert_eq!(determine_ext(None, "custom"), "custom");

        let mut info = InfoDict::new();
        info.insert("url", json!("https://example.test/manifest.m3u8"));
        assert_eq!(determine_protocol(&info).unwrap(), "m3u8_native");
        info.insert("is_live", json!(true));
        assert_eq!(determine_protocol(&info).unwrap(), "m3u8");
        info.insert("protocol", json!("http_dash_segments"));
        assert_eq!(determine_protocol(&info).unwrap(), "http_dash_segments");

        assert_eq!(int_or_none(Some(&json!("1536")), 1024, 1, None), Some(1));
        assert_eq!(int_or_none(Some(&json!(-3)), 2, 1, None), Some(-2));
        assert_eq!(float_or_none(Some(&json!("1.5")), 2.0, 1.0), Some(0.75));
        assert_eq!(
            str_or_none(Some(&json!(true)), None),
            Some("True".to_owned())
        );
        assert_eq!(
            str_or_none(None, Some("fallback")),
            Some("fallback".to_owned())
        );
    }
}
