/// Native 1TV VOD playlist extractor.
pub struct FirstTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FirstTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FirstTvExtractor {
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
        let display_id = firsttv_match_id(&self.matcher, url, "1TV")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let playlist_raw = Regex::new(
            r#"(?is)\bdata-playlist-url\s*=\s*["']([^"']+)["']"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("1TV page {display_id} has no playlist URL"),
            )
        })?;
        let playlist_url = resolve_url(url, &playlist_raw);
        let playlist = context.get_json(&playlist_url)?;
        let selected_ids = firsttv_selected_ids(&playlist_url);
        let items = playlist.as_array().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("1TV playlist {display_id} is not an array"),
            )
        })?;
        let mut entries = Vec::new();
        for item in items {
            let uid = json_value_string(item.get("uid"));
            if let Some(selected_ids) = selected_ids.as_ref() {
                if uid
                    .as_deref()
                    .is_none_or(|item_id| !selected_ids.iter().any(|id| id == item_id))
                {
                    continue;
                }
            }
            if let Some(entry) = firsttv_entry(item, &playlist_url) {
                entries.push(entry);
            }
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("1TV playlist {display_id} has no matching entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(display_id));
        info.insert_if_some(
            "title",
            html_meta_value(&webpage, "og:title").or_else(|| html_title_value(&webpage)),
        );
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native 1TV live-channel DASH extractor.
pub struct FirstTvLiveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FirstTvLiveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FirstTvLiveExtractor {
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
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let stream_data =
            context.get_json("https://stream.1tv.ru/api/playlist/1tvch-v1_as_array.json")?;
        let mpd_url = stream_data
            .get("mpd")
            .and_then(firsttv_http_url)
            .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "1TV live channel has no DASH manifest",
            )
        })?;
        let title = html_title_value(&webpage).unwrap_or_else(|| "1TV live".to_owned());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!("live"));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(mpd_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": mpd_url,
                "format_id": "dash",
                "protocol": "http_dash_segments",
                "ext": "mp4",
                "is_live": true,
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        info.insert("is_live", serde_json::json!(true));
        Ok(ExtractorResult::single(info))
    }
}

fn firsttv_entry(item: &serde_json::Value, base_url: &str) -> Option<InfoDict> {
    let video_id = json_value_string(item.get("id"))
        .or_else(|| json_value_string(item.get("uid")))?;
    let mut formats = Vec::new();
    if let Some(sources) = item.get("sources").and_then(serde_json::Value::as_array) {
        for source in sources {
            let Some(raw_url) = json_string(source, "src") else {
                continue;
            };
            let media_url = resolve_url(base_url, raw_url);
            let extension = firsttv_source_extension(source, &media_url);
            let (protocol, ext) = match extension.as_str() {
                "m3u8" => ("m3u8_native", "mp4"),
                "mpd" => ("http_dash_segments", "mp4"),
                _ => ("http", extension.as_str()),
            };
            let tbr = Regex::new(r#"_(\d{3,})\.[^./?]+(?:[?#]|$)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&media_url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<i64>().ok());
            let format_id = tbr.map_or_else(
                || format!("http-{ext}"),
                |value| format!("http-{ext}-{value}"),
            );
            let mut format = serde_json::json!({
                "url": media_url,
                "ext": ext,
                "format_id": format_id,
                "protocol": protocol,
                "quality": -10,
            });
            if let Some(tbr) = tbr {
                format["tbr"] = serde_json::json!(tbr);
            }
            formats.push(format);
        }
    }
    if formats.is_empty() {
        return None;
    }
    let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert_if_some("title", json_string(item, "title"));
    info.insert_if_some("thumbnail", json_string(item, "poster"));
    info.insert_if_some("timestamp", json_i64(item, "dvr_begin_at"));
    info.insert_if_some("upload_date", json_string(item, "date_air").and_then(date_digits));
    info.insert_if_some("duration", json_i64(item, "duration"));
    if let Some(chapters) = firsttv_chapters(item) {
        info.insert("chapters", chapters);
    }
    info.insert(
        "url",
        first_format
            .get("url")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    info.insert(
        "ext",
        first_format
            .get("ext")
            .cloned()
            .unwrap_or_else(|| serde_json::json!("mp4")),
    );
    info.insert("formats", serde_json::Value::Array(formats));
    info.insert("subtitles", serde_json::json!({}));
    Some(info)
}

fn firsttv_source_extension(source: &serde_json::Value, media_url: &str) -> String {
    let mime = json_string(source, "type").unwrap_or_default().to_ascii_lowercase();
    if mime.contains("mpegurl") || mime.contains("m3u8") {
        return "m3u8".to_owned();
    }
    if mime.contains("dash") || mime.contains("mpd") {
        return "mpd".to_owned();
    }
    yt_dlp_core::determine_ext(Some(media_url), "mp4")
}

fn firsttv_chapters(item: &serde_json::Value) -> Option<serde_json::Value> {
    let chapters = item
        .get("episodes")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|episode| {
            let start_time = json_f64(episode, "from")?;
            let mut chapter = serde_json::json!({"start_time": start_time});
            if let Some(end_time) = json_f64(episode, "to") {
                chapter["end_time"] = serde_json::json!(end_time);
            }
            if let Some(title) = json_string(episode, "name") {
                chapter["title"] = serde_json::json!(html_text_fragment(title));
            }
            Some(chapter)
        })
        .collect::<Vec<_>>();
    (!chapters.is_empty()).then(|| serde_json::Value::Array(chapters))
}

fn firsttv_selected_ids(playlist_url: &str) -> Option<Vec<String>> {
    let parsed = url::Url::parse(playlist_url).ok()?;
    let selected = parsed
        .query_pairs()
        .filter(|(key, _)| matches!(key.as_ref(), "video_id" | "videos_ids[]" | "news_ids[]"))
        .map(|(_, value)| value.into_owned())
        .collect::<Vec<_>>();
    (!selected.is_empty()).then_some(selected)
}

fn firsttv_http_url(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value)
            if value.starts_with("http://") || value.starts_with("https://") =>
        {
            Some(value.to_owned())
        }
        serde_json::Value::Array(values) => values.iter().find_map(firsttv_http_url),
        serde_json::Value::Object(values) => values.values().find_map(firsttv_http_url),
        _ => None,
    }
}

fn firsttv_match_id(
    matcher: &Regex,
    url: &str,
    label: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("{label} URL has no display ID"),
            )
        })
}
