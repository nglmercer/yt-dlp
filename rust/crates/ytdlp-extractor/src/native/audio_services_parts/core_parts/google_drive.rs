/// Native Google Drive playback extractor. Playback JSON formats and the
/// source-download response are handled with the Rust request stack.
pub struct GoogleDriveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GoogleDriveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GoogleDriveExtractor {
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
                "Google Drive URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Google Drive URL has no ID")
            })?;
        let mut playback_request = Request::new(format!(
            "https://content-workspacevideo-pa.googleapis.com/v1/drive/media/{video_id}/playback"
        ));
        playback_request.update_query(&[(
            "key".to_owned(),
            "AIzaSyDVQw45DwoYh632gvsP5vPDqEKvb-Ywnb8".to_owned(),
        )]);
        playback_request
            .headers_mut()
            .set("Referer", "https://drive.google.com/");
        let playback_response = context.request(&playback_request)?;
        let video_info: serde_json::Value = serde_json::from_slice(playback_response.body())
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Google Drive playback JSON: {error}"),
                )
            })?;

        let streaming_data = video_info
            .get("mediaStreamingData")
            .and_then(|value| value.get("formatStreamingData"));
        let mut formats = Vec::new();
        for group in ["adaptiveTranscodes", "progressiveTranscodes"] {
            let Some(transcodes) = streaming_data
                .and_then(|value| value.get(group))
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for transcode in transcodes {
                let Some(media_url) = json_string(transcode, "url") else {
                    continue;
                };
                let metadata = transcode.get("transcodeMetadata");
                let ext = google_drive_mime_extension(
                    metadata.and_then(|value| json_string(value, "mimeType")),
                )
                .unwrap_or("mp4");
                let mut format = serde_json::Map::new();
                format.insert("url".to_owned(), serde_json::json!(media_url));
                format.insert(
                    "format_id".to_owned(),
                    serde_json::json!(
                        json_value_string(transcode.get("itag"))
                            .unwrap_or_else(|| group.to_owned())
                    ),
                );
                format.insert("ext".to_owned(), serde_json::json!(ext));
                for (source, target) in [
                    ("width", "width"),
                    ("height", "height"),
                    ("videoFps", "fps"),
                    ("contentLength", "filesize"),
                ] {
                    if let Some(value) = metadata.and_then(|value| value.get(source)) {
                        format.insert(target.to_owned(), value.clone());
                    }
                }
                if let Some(value) =
                    metadata.and_then(|value| json_string(value, "videoCodecString"))
                {
                    format.insert("vcodec".to_owned(), serde_json::json!(value));
                }
                if let Some(value) =
                    metadata.and_then(|value| json_string(value, "audioCodecString"))
                {
                    format.insert("acodec".to_owned(), serde_json::json!(value));
                }
                format.insert(
                    "downloader_options".to_owned(),
                    serde_json::json!({"http_chunk_size": 10 << 20}),
                );
                formats.push(serde_json::Value::Object(format));
            }
        }

        let mut title = video_info
            .get("mediaMetadata")
            .and_then(|value| json_string(value, "title"))
            .map(str::to_owned);
        let source_response = {
            let mut request = Request::new("https://drive.usercontent.google.com/download");
            request.update_query(&[
                ("id".to_owned(), video_id.to_owned()),
                ("export".to_owned(), "download".to_owned()),
                ("confirm".to_owned(), "t".to_owned()),
            ]);
            request
                .headers_mut()
                .set("Referer", "https://drive.google.com/");
            context.request(&request).ok()
        };
        if let Some(response) = source_response {
            if let Some(filename) =
                google_drive_filename(response.headers().get("Content-Disposition"))
            {
                title.get_or_insert(filename);
                let ext = title
                    .as_deref()
                    .map(|value| yt_dlp_core::determine_ext(Some(value), "mp4"))
                    .unwrap_or_else(|| "mp4".to_owned());
                formats.push(serde_json::json!({
                    "url": response.url(),
                    "format_id": "source",
                    "ext": ext,
                    "quality": 1,
                    "protocol": "https",
                }));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Google Drive file {video_id} has no playable formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
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
        if let Some(duration) = video_info.get("mediaMetadata").and_then(|value| {
            json_f64(value, "duration")
                .or_else(|| json_string(value, "duration").and_then(yt_dlp_core::parse_duration))
        }) {
            info.insert("duration", serde_json::json!(duration));
        }
        if let Some(thumbnails) = video_info
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
        {
            let thumbnails = thumbnails
                .iter()
                .filter_map(|thumbnail| {
                    let url = json_string(thumbnail, "url")?;
                    let mut value = serde_json::json!({"url": url});
                    for key in ["width", "height"] {
                        if let Some(number) = thumbnail.get(key) {
                            value[key] = number.clone();
                        }
                    }
                    Some(value)
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}
