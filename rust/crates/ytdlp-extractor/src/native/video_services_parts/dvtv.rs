/// Native DVTV player-object extractor.
pub struct DvtvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DvtvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DvtvExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DVTV URL has no video ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let timestamp = html_meta_value(&webpage, "article:published_time").and_then(parse_timestamp);

        let playlist_marker = "playlist.push(";
        let playlist_items = json_objects_after_marker(&webpage, playlist_marker);
        if !playlist_items.is_empty() {
            let entries = playlist_items
                .iter()
                .map(|item| dvtv_video_info(item, &video_id, timestamp))
                .collect::<Result<Vec<_>, _>>()?;
            let title = html_meta_value(&webpage, "twitter:title")
                .map(|value| unescape_html_attribute(&value))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| video_id.clone());
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(video_id));
            info.insert("title", serde_json::json!(title));
            return Ok(ExtractorResult::Playlist { info, entries });
        }
        if webpage.contains(playlist_marker) {
            return Err(dvtv_javascript_todo(
                &video_id,
                "playlist.push() contains a JavaScript expression the native parser cannot evaluate",
            ));
        }

        let player_marker = "BBXPlayer.setup(";
        if let Some(item) = json_object_after_marker(&webpage, player_marker) {
            return Ok(ExtractorResult::single(dvtv_video_info(
                &item, &video_id, timestamp,
            )?));
        }
        if webpage.contains(player_marker) {
            return Err(dvtv_javascript_todo(
                &video_id,
                "BBXPlayer.setup() contains a JavaScript expression the native parser cannot evaluate",
            ));
        }

        Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("DVTV page {video_id} has no video or playlist player object"),
        ))
    }
}

fn dvtv_video_info(
    data: &serde_json::Value,
    page_id: &str,
    timestamp: Option<i64>,
) -> Result<InfoDict, ExtractorError> {
    let mut data = data.clone();
    if let Some(live_starter) = data
        .get("plugins")
        .and_then(|plugins| plugins.get("liveStarter"))
        .and_then(serde_json::Value::as_object)
        .cloned()
    {
        let object = data.as_object_mut().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DVTV player {page_id} has a non-object payload"),
            )
        })?;
        object.extend(live_starter);
    }
    let title = json_string(&data, "title")
        .map(unescape_html_attribute)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DVTV player {page_id} has no title"),
            )
        })?;
    let video_id = json_string(&data, "mediaid")
        .or_else(|| json_string(&data, "video_id"))
        .filter(|value| !value.is_empty())
        .unwrap_or(page_id)
        .to_owned();
    let mut formats = Vec::new();
    if let Some(tracks) = data.get("tracks").and_then(serde_json::Value::as_object) {
        for track_list in tracks.values() {
            for video in track_list
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_object)
            {
                let Some(raw_url) = video
                    .get("src")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let media_url = proto_relative_url(raw_url, "https:");
                let media_type = video.get("type").and_then(serde_json::Value::as_str);
                let fallback_ext = mimetype_extension(media_type).unwrap_or_else(|| "unknown".to_owned());
                let detected_ext = yt_dlp_core::determine_ext(Some(&media_url), &fallback_ext);
                let is_hls = media_type == Some("application/vnd.apple.mpegurl")
                    || detected_ext.eq_ignore_ascii_case("m3u8");
                let is_dash = media_type == Some("application/dash+xml")
                    || detected_ext.eq_ignore_ascii_case("mpd");
                if is_hls || is_dash {
                    formats.push(serde_json::json!({
                        "url": media_url,
                        "format_id": if is_hls { "hls" } else { "dash" },
                        "protocol": if is_hls { "m3u8_native" } else { "http" },
                        "ext": "mp4",
                    }));
                    continue;
                }
                let label = video
                    .get("label")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty());
                let format_id = label.map_or_else(
                    || format!("http-{detected_ext}"),
                    |label| format!("http-{detected_ext}-{label}"),
                );
                let height = label
                    .and_then(|label| Regex::new(r"^(\d+)[pP]").ok()?.captures(label).ok().flatten())
                    .and_then(|captures| captures.get(1))
                    .and_then(|value| value.as_str().parse::<i64>().ok());
                let mut format = serde_json::Map::new();
                format.insert("url".to_owned(), serde_json::json!(media_url));
                format.insert("format_id".to_owned(), serde_json::json!(format_id));
                format.insert("ext".to_owned(), serde_json::json!(detected_ext));
                format.insert("protocol".to_owned(), serde_json::json!("http"));
                if let Some(height) = height {
                    format.insert("height".to_owned(), serde_json::json!(height));
                }
                formats.push(serde_json::Value::Object(format));
            }
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("DVTV player {video_id} has no playable tracks"),
        ));
    }
    let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert("title", serde_json::json!(title));
    info.insert_if_some(
        "description",
        json_string(&data, "description").map(unescape_html_attribute),
    );
    info.insert_if_some(
        "thumbnail",
        json_string(&data, "image").map(|value| proto_relative_url(value, "https:")),
    );
    info.insert_if_some("duration", json_i64(&data, "duration"));
    info.insert_if_some("timestamp", timestamp);
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
    Ok(info)
}

fn dvtv_javascript_todo(video_id: &str, detail: &str) -> ExtractorError {
    ExtractorError::new(
        ExtractorErrorKind::Unsupported,
        format!("TODO: DVTV video {video_id} requires native JavaScript support: {detail}"),
    )
}
