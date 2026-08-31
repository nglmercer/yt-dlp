/// Native Stream.cz/Televize Seznam GraphQL and playlist extractor.
pub struct StreamCzExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl StreamCzExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for StreamCzExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matcher.is_match(url).unwrap_or(false)
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        1
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Stream.cz URL did not match its native pattern",
            )
        })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Stream.cz URL has no slug")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Stream.cz URL has no ID")
            })?;
        let graphql_payload = serde_json::json!({
            "variables": {"urlName": video_id},
            "query": "query LoadEpisode($urlName : String){ episode(urlName: $urlName){ id spl urlName name perex duration views } }"
        });
        let graphql = native_post_json(
            context,
            "https://www.televizeseznam.cz/api/graphql",
            &graphql_payload,
        )?;
        let episode = graphql
            .get("data")
            .and_then(|data| data.get("episode"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Stream.cz episode {video_id} is missing from GraphQL response"),
                )
            })?;
        let playlist_base = json_string(episode, "spl").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Stream.cz episode {video_id} has no playlist URL"),
            )
        })?;
        let playlist_url = format!("{playlist_base}spl2,3");
        let mut playlist = context.get_json(&playlist_url)?;
        if playlist.get("data").is_none() {
            if let Some(location) = json_string(&playlist, "Location") {
                playlist = context.get_json(location)?;
            }
        }
        let video = playlist.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Stream.cz playlist {playlist_url} has no data"),
            )
        })?;
        let mut formats = Vec::new();
        if let Some(qualities) = video
            .get("http_stream")
            .and_then(|stream| stream.get("qualities"))
            .and_then(serde_json::Value::as_object)
        {
            for (format_id, stream) in qualities {
                add_stream_cz_format(&playlist_url, format_id, stream, "ts", -1, &mut formats);
            }
        }
        if let Some(qualities) = video.get("mp4").and_then(serde_json::Value::as_object) {
            for (format_id, stream) in qualities {
                add_stream_cz_format(&playlist_url, format_id, stream, "mp4", 1, &mut formats);
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Stream.cz episode {video_id} has no playable formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut subtitles = serde_json::Map::new();
        if let Some(values) = video
            .get("subtitles")
            .and_then(serde_json::Value::as_object)
        {
            for subtitle in values.values() {
                let Some(language) = json_string(subtitle, "language") else {
                    continue;
                };
                let Some(urls) = subtitle.get("urls").and_then(serde_json::Value::as_object) else {
                    continue;
                };
                let entries = urls
                    .iter()
                    .filter_map(|(extension, value)| {
                        let media_url = value.as_str()?;
                        Some(serde_json::json!({
                            "ext": extension,
                            "url": resolve_url(&playlist_url, media_url),
                        }))
                    })
                    .collect::<Vec<_>>();
                if !entries.is_empty() {
                    subtitles.insert(language.to_owned(), serde_json::Value::Array(entries));
                }
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(episode, "name"));
        info.insert_if_some("description", json_string(episode, "perex"));
        info.insert_if_some("duration", json_f64(episode, "duration"));
        info.insert_if_some("view_count", json_i64(episode, "views"));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        if !subtitles.is_empty() {
            info.insert("subtitles", serde_json::Value::Object(subtitles));
        }
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn add_stream_cz_format(
    playlist_url: &str,
    format_id: &str,
    stream: &serde_json::Value,
    extension: &str,
    source_preference: i64,
    formats: &mut Vec<serde_json::Value>,
) {
    let Some(raw_url) = json_string(stream, "url") else {
        return;
    };
    let mut format = serde_json::json!({
        "format_id": format!("{format_id}-{extension}"),
        "ext": extension,
        "source_preference": source_preference,
        "url": resolve_url(playlist_url, raw_url),
    });
    if let Some(value) = json_f64(stream, "bandwidth") {
        format["tbr"] = serde_json::json!(value / 1000.0);
    }
    if let Some(value) = json_f64(stream, "duration") {
        format["duration"] = serde_json::json!(value / 1000.0);
    }
    if let Some(resolution) = stream
        .get("resolution")
        .and_then(serde_json::Value::as_array)
    {
        if let Some(width) = resolution.first().and_then(serde_json::Value::as_i64) {
            format["width"] = serde_json::json!(width);
        }
        if let Some(height) = resolution.get(1).and_then(serde_json::Value::as_i64) {
            format["height"] = serde_json::json!(height);
        }
    }
    if format.get("height").is_none() {
        if let Ok(height) = format_id.trim_end_matches('p').parse::<i64>() {
            format["height"] = serde_json::json!(height);
        }
    }
    if let Some(codec) = json_string(stream, "codec") {
        let codec = codec.to_ascii_lowercase();
        if codec.contains("avc") || codec.contains("h264") || codec.contains("vp8") {
            format["vcodec"] = serde_json::json!(codec);
        }
        if codec.contains("aac") || codec.contains("mp4a") || codec.contains("opus") {
            format["acodec"] = serde_json::json!(codec);
        }
    }
    formats.push(format);
}

fn resolve_url(base: &str, value: &str) -> String {
    url::Url::parse(base)
        .ok()
        .and_then(|base| base.join(value).ok())
        .map(|value| value.to_string())
        .unwrap_or_else(|| value.to_owned())
}

fn xml_element_text(xml: &str, element: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<{}\b[^>]*>(.*?)</{}\s*>"#,
        regex::escape(element),
        regex::escape(element)
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(xml)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn add_peertube_file_format(file: &serde_json::Value, formats: &mut Vec<serde_json::Value>) {
    let Some(file_url) = json_string(file, "fileUrl") else {
        return;
    };
    let label = file
        .get("resolution")
        .and_then(|resolution| json_string(resolution, "label"));
    let mut format = serde_json::json!({
        "url": file_url,
        "format_id": label,
        "filesize": json_i64(file, "size"),
        "ext": yt_dlp_core::determine_ext(Some(file_url), "mp4"),
    });
    if let Some(label) = label {
        if let Some((width, height)) = parse_resolution_label(label) {
            format["width"] = serde_json::json!(width);
            format["height"] = serde_json::json!(height);
        } else if label.ends_with('p') {
            if let Ok(height) = label.trim_end_matches('p').parse::<i64>() {
                format["height"] = serde_json::json!(height);
            }
        }
        if label == "0p" {
            format["vcodec"] = serde_json::json!("none");
        } else if let Some(fps) = json_i64(file, "fps") {
            format["fps"] = serde_json::json!(fps);
        }
    }
    if format.get("ext").and_then(serde_json::Value::as_str) == Some("m3u8") {
        format["ext"] = serde_json::json!("mp4");
        format["protocol"] = serde_json::json!("m3u8_native");
    }
    formats.push(format);
}

fn parse_resolution_label(label: &str) -> Option<(i64, i64)> {
    let matcher = Regex::new(r#"(?i)^(\d+)x(\d+)$"#).ok()?;
    let captures = matcher.captures(label).ok().flatten()?;
    Some((
        captures.get(1)?.as_str().parse().ok()?,
        captures.get(2)?.as_str().parse().ok()?,
    ))
}

fn html_element_by_class(html: &str, class: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<([a-z0-9]+)\b[^>]*\bclass\s*=\s*["'][^"']*\b{}\b[^"']*["'][^>]*>(.*?)</\1\s*>"#,
        regex::escape(class)
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(2).map(|value| value.as_str().to_owned()))
}

fn html_field_value(html: &str, field_name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<span\s+class\s*=\s*["']field_title["'][^>]*>\s*{}\s*:\s*</span>\s*<span\s+class\s*=\s*["']field_content["'][^>]*>([^<]+)"#,
        regex::escape(field_name)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn peertube_subtitles(
    host: &str,
    video_id: &str,
    context: &ExtractionContext,
) -> Option<serde_json::Value> {
    let captions = context
        .get_json(&format!("https://{host}/api/v1/videos/{video_id}/captions"))
        .ok()?;
    let data = captions.get("data").and_then(serde_json::Value::as_array)?;
    let mut subtitles = serde_json::Map::new();
    for caption in data {
        let Some(path) = json_string(caption, "captionPath") else {
            continue;
        };
        let language = caption
            .get("language")
            .and_then(|language| json_string(language, "id"))
            .unwrap_or("en");
        subtitles.insert(
            language.to_owned(),
            serde_json::json!([{"url": format!("https://{host}{path}")}]),
        );
    }
    (!subtitles.is_empty()).then_some(serde_json::Value::Object(subtitles))
}
