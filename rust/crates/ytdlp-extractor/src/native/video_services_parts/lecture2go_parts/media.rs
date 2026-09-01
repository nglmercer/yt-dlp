fn lecture2go_formats(
    webpage: &str,
    video_id: &str,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let matcher = Regex::new(r#"var\s+playerUri\d+\s*=\s*"([^"]+)""#).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Lecture2Go player URL matcher: {error}"),
        )
    })?;
    let mut formats = Vec::new();
    let mut seen_urls = Vec::new();
    let mut saw_unsupported = false;
    for captures in matcher.captures_iter(webpage).flatten() {
        let Some(media_url) = captures.get(1).map(|value| value.as_str().to_owned()) else {
            continue;
        };
        if !seen_urls.insert_unique(media_url.clone()) {
            continue;
        }
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "unknown").to_ascii_lowercase();
        if extension == "f4m" {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Lecture2Go video {video_id} requires Adobe HDS/F4M manifest parsing"
                ),
            ));
        }
        if media_url.starts_with("rtmp://") || media_url.starts_with("rtmps://") {
            saw_unsupported = true;
            continue;
        }
        let (format_id, protocol, ext) = if extension == "m3u8" {
            ("hls".to_owned(), "m3u8_native".to_owned(), "mp4".to_owned())
        } else {
            let protocol = url::Url::parse(&media_url)
                .ok()
                .map(|url| url.scheme().to_owned())
                .filter(|scheme| !scheme.is_empty())
                .unwrap_or_else(|| "http".to_owned());
            (protocol.clone(), protocol, extension)
        };
        let mut format = serde_json::json!({
            "format_id": format_id,
            "url": media_url,
        });
        if protocol == "m3u8_native" {
            format["protocol"] = serde_json::json!("m3u8_native");
            format["ext"] = serde_json::json!(ext);
        } else if ext != "unknown" {
            format["ext"] = serde_json::json!(ext);
        }
        formats.push(format);
    }
    if formats.is_empty() && saw_unsupported {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Lecture2Go video {video_id} has only unsupported RTMP sources"
            ),
        ));
    }
    Ok(formats)
}

trait InsertUnique<T> {
    fn insert_unique(&mut self, value: T) -> bool;
}

impl<T: PartialEq> InsertUnique<T> for Vec<T> {
    fn insert_unique(&mut self, value: T) -> bool {
        if self.contains(&value) {
            false
        } else {
            self.push(value);
            true
        }
    }
}
