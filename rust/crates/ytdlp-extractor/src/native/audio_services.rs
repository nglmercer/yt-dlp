/// Native AudioBoom HTML/API extractor. The page embeds the same clip store
/// used by the source implementation; Rust reads that JSON directly and
/// falls back to Open Graph/audio metadata when the player attributes change.
pub struct AudioBoomExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudioBoomExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AudioBoomExtractor {
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
                "AudioBoom URL did not match its native pattern",
            )
        })?;
        let audio_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "AudioBoom URL has no ID")
            })?;
        let webpage = context.get(&format!("https://audioboom.com/posts/{audio_id}"))?;
        let html = String::from_utf8_lossy(webpage.body());
        let clip_store = audio_boom_clip_store(&html);
        let clip = clip_store
            .as_ref()
            .and_then(|store| store.get("clips"))
            .and_then(serde_json::Value::as_array)
            .and_then(|clips| clips.first());

        let media_url = clip
            .and_then(|clip| json_string(clip, "clipURLPriorToLoading"))
            .map(str::to_owned)
            .or_else(|| {
                html_meta_value(&html, "og:audio").map(|value| unescape_html_attribute(&value))
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("AudioBoom page has no playable audio for {audio_id}"),
                )
            })?;
        let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp3");
        let title = clip
            .and_then(|clip| json_string(clip, "title"))
            .map(str::to_owned)
            .or_else(|| {
                ["og:title", "og:audio:title", "audio_title"]
                    .iter()
                    .find_map(|key| html_meta_value(&html, key))
            })
            .unwrap_or_else(|| audio_id.to_owned());
        let description = clip
            .and_then(|clip| json_string(clip, "description"))
            .map(str::to_owned)
            .or_else(|| {
                clip.and_then(|clip| json_string(clip, "formattedDescription"))
                    .map(html_text_fragment)
            })
            .or_else(|| html_meta_value(&html, "og:description"));
        let duration = clip
            .and_then(|clip| json_f64(clip, "duration"))
            .or_else(|| {
                html_meta_value(&html, "weibo:audio:duration")
                    .and_then(|value| value.parse::<f64>().ok())
            });
        let uploader = clip
            .and_then(|clip| json_string(clip, "author"))
            .map(str::to_owned)
            .or_else(|| {
                [
                    "og:audio:artist",
                    "twitter:audio:artist_name",
                    "audio_artist",
                ]
                .iter()
                .find_map(|key| html_meta_value(&html, key))
            });
        let uploader_url = Regex::new(
            r#"(?is)<div\b[^>]*class\s*=\s*["'][^"']*\bavatar\b[^"']*["'][^>]*>.*?<a\b[^>]*href\s*=\s*["'](https?://[^"']+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()));

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("uploader", uploader);
        info.insert_if_some("uploader_url", uploader_url);
        Ok(ExtractorResult::single(info))
    }
}

/// Native BitChute API extractor. Video media and metadata are obtained from
/// the public JSON endpoints; HLS URLs are handed to the native downloader.
pub struct BitChuteExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BitChuteExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BitChuteExtractor {
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
                "BitChute URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "BitChute URL has no ID")
            })?;
        let payload = serde_json::json!({"video_id": video_id});
        let media = native_post_json(
            context,
            "https://api.bitchute.com/api/beta/video/media",
            &payload,
        )?;
        let media_url = json_string(&media, "media_url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "BitChute media response has no media_url",
            )
        })?;
        let detected_ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let is_hls = detected_ext == "m3u8";
        let output_ext = if is_hls {
            "mp4".to_owned()
        } else {
            detected_ext
        };
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(output_ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": if is_hls { "hls" } else { "direct" },
                "ext": output_ext,
                "protocol": if is_hls { "m3u8_native" } else { "http" },
            }]),
        );

        let video =
            native_post_json(context, "https://api.bitchute.com/api/beta/video", &payload).ok();
        if let Some(video) = video.as_ref() {
            info.insert_if_some("title", json_string(video, "video_name"));
            info.insert_if_some("description", json_string(video, "description"));
            info.insert_if_some("thumbnail", json_string(video, "thumbnail_url"));
            info.insert_if_some("view_count", json_i64(video, "view_count"));
            let duration = json_f64(video, "duration")
                .or_else(|| json_string(video, "duration").and_then(yt_dlp_core::parse_duration));
            info.insert_if_some("duration", duration);
            if let Some(value) = video.get("date_published") {
                info.insert("date_published", value.clone());
            }
            if let Some(value) = video.get("state_id").and_then(serde_json::Value::as_str) {
                info.insert("is_live", serde_json::json!(value == "live"));
            }
            if let Some(tags) = video.get("hashtags").and_then(serde_json::Value::as_array) {
                let tags = tags
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .map(serde_json::Value::String)
                    .collect::<Vec<_>>();
                info.insert("tags", serde_json::Value::Array(tags));
            }
            if let Some(profile_id) = json_string(video, "profile_id") {
                info.insert("uploader_id", serde_json::json!(profile_id));
                info.insert(
                    "uploader_url",
                    serde_json::json!(format!("https://www.bitchute.com/profile/{profile_id}/")),
                );
            }
            if let Some(channel) = video.get("channel") {
                info.insert_if_some("channel", json_string(channel, "channel_name"));
                info.insert_if_some("channel_id", json_string(channel, "channel_id"));
                if let Some(channel_url) = json_string(channel, "channel_url") {
                    info.insert("channel_url", serde_json::json!(channel_url));
                }
                if let Some(channel_id) = json_string(channel, "channel_id") {
                    if let Ok(channel_data) = native_post_json(
                        context,
                        "https://api.bitchute.com/api/beta/channel",
                        &serde_json::json!({"channel_id": channel_id}),
                    ) {
                        info.insert_if_some("uploader", json_string(&channel_data, "profile_name"));
                        info.insert_if_some(
                            "uploader_id",
                            json_string(&channel_data, "profile_id"),
                        );
                        if let Some(profile_id) = json_string(&channel_data, "profile_id") {
                            info.insert(
                                "uploader_url",
                                serde_json::json!(format!(
                                    "https://www.bitchute.com/profile/{profile_id}/"
                                )),
                            );
                        }
                        info.insert_if_some("channel", json_string(&channel_data, "channel_name"));
                        if let Some(slug) = json_string(&channel_data, "url_slug") {
                            info.insert(
                                "channel_url",
                                serde_json::json!(format!(
                                    "https://www.bitchute.com/channel/{slug}/"
                                )),
                            );
                        }
                    }
                }
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

fn archive_download_url(identifier: &str, name: &str) -> String {
    let mut url = url::Url::parse("https://archive.org/download").expect("static Archive.org URL");
    {
        let mut segments = url
            .path_segments_mut()
            .expect("Archive.org URL has mutable path segments");
        segments.push(identifier);
        segments.push(name);
    }
    url.to_string()
}

fn decode_url_component(value: &str) -> String {
    fn hex_digit(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn archive_text_value(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|value| {
            value.as_str().map(str::to_owned).or_else(|| {
                value.as_array().map(|values| {
                    values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
            })
        })
        .filter(|value| !value.is_empty())
}

fn archive_file_extension(name: &str) -> Option<String> {
    let extension = name.rsplit_once('.')?.1.trim().to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "3gp"
            | "aac"
            | "aiff"
            | "ape"
            | "avi"
            | "flac"
            | "flv"
            | "m4a"
            | "m4v"
            | "mka"
            | "mkv"
            | "mov"
            | "mp3"
            | "mp4"
            | "mpa"
            | "mpeg"
            | "mpg"
            | "oga"
            | "ogg"
            | "ogv"
            | "opus"
            | "wav"
            | "webm"
            | "wmv"
    )
    .then_some(extension)
}

/// Native Archive.org metadata extractor. Archive items are represented from
/// the public metadata JSON, with files sharing their 'original' name grouped
/// into one entry and multiple media entries returned as a native playlist.
pub struct ArchiveOrgExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ArchiveOrgExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ArchiveOrgExtractor {
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
                "Archive.org URL did not match its native pattern",
            )
        })?;
        let requested_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Archive.org URL has no ID")
            })?;
        let requested_id = decode_url_component(requested_id);
        let (requested_identifier, requested_entry) = requested_id
            .split_once('/')
            .map_or((requested_id.clone(), None), |(identifier, entry)| {
                (identifier.to_owned(), Some(entry.to_owned()))
            });
        let metadata = context.get_json(&format!(
            "https://archive.org/metadata/{requested_identifier}"
        ))?;
        let metadata_info = metadata.get("metadata").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Archive.org metadata has no metadata object",
            )
        })?;
        let identifier = json_string(metadata_info, "identifier")
            .unwrap_or(requested_identifier.as_str())
            .to_owned();

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(identifier));
        info.insert(
            "webpage_url",
            serde_json::json!(format!("https://archive.org/details/{identifier}")),
        );
        info.insert_if_some("title", archive_text_value(metadata_info.get("title")));
        info.insert_if_some(
            "description",
            archive_text_value(metadata_info.get("description")),
        );
        info.insert_if_some(
            "uploader",
            archive_text_value(
                metadata_info
                    .get("uploader")
                    .or_else(|| metadata_info.get("adder")),
            ),
        );
        info.insert_if_some("license", json_string(metadata_info, "licenseurl"));
        info.insert_if_some("location", json_string(metadata_info, "venue"));
        info.insert_if_some("release_year", json_i64(metadata_info, "year"));
        info.insert_if_some("release_date", json_string(metadata_info, "date"));
        if let Some(value) = metadata_info.get("creator") {
            info.insert("creators", value.clone());
        }

        let files = metadata
            .get("files")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Archive.org metadata has no files array",
                )
            })?;
        let mut entries = Vec::<InfoDict>::new();
        for file in files {
            if json_string(file, "format") == Some("Thumbnail") {
                continue;
            }
            let Some(name) = json_string(file, "name") else {
                continue;
            };
            let Some(extension) = archive_file_extension(name) else {
                continue;
            };
            let group = json_string(file, "original").unwrap_or(name);
            if let Some(requested_entry) = requested_entry.as_deref()
                && requested_entry != name
                && requested_entry != group
            {
                continue;
            }
            let entry_index = entries
                .iter()
                .position(|entry| entry.get_str("_archive_group") == Some(group))
                .unwrap_or_else(|| {
                    let mut entry = InfoDict::new();
                    entry.insert("_archive_group", serde_json::json!(group));
                    entry.insert("id", serde_json::json!(format!("{identifier}/{group}")));
                    entry.insert("display_id", serde_json::json!(group));
                    entry.insert(
                        "title",
                        serde_json::json!(json_string(file, "title").unwrap_or(group)),
                    );
                    entry.insert("formats", serde_json::json!([]));
                    entries.push(entry);
                    entries.len() - 1
                });
            let entry = &mut entries[entry_index];
            if let Some(value) = json_string(file, "description") {
                if !entry.contains_key("description") {
                    entry.insert("description", serde_json::json!(value));
                }
            }
            if let Some(value) = json_string(file, "creator") {
                if !entry.contains_key("creators") {
                    entry.insert("creators", serde_json::json!([value]));
                }
            }
            entry.insert_if_some(
                "duration",
                json_f64(file, "length")
                    .or_else(|| json_string(file, "length").and_then(yt_dlp_core::parse_duration)),
            );
            entry.insert_if_some("track_number", json_i64(file, "track"));
            entry.insert_if_some("album", json_string(file, "album"));
            entry.insert_if_some("discnumber", json_i64(file, "disc"));
            let file_url = archive_download_url(&identifier, name);
            let format = serde_json::json!({
                "url": file_url,
                "format": file.get("format").cloned().unwrap_or(serde_json::Value::Null),
                "ext": extension,
                "width": json_i64(file, "width"),
                "height": json_i64(file, "height"),
                "filesize": json_i64(file, "size"),
                "protocol": "https",
                "format_note": file.get("source").cloned().unwrap_or(serde_json::Value::Null),
            });
            let mut formats = entry
                .remove("formats")
                .and_then(|value| value.as_array().cloned())
                .unwrap_or_default();
            formats.push(format);
            entry.insert("formats", serde_json::Value::Array(formats));
            if !entry.contains_key("url") {
                entry.insert("url", serde_json::json!(file_url));
                entry.insert("ext", serde_json::json!(extension));
            }
        }
        for entry in &mut entries {
            entry.remove("_archive_group");
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Archive.org item {identifier} has no playable media files"),
            ));
        }

        if let Some(requested_entry) = requested_entry.as_deref() {
            let selected = entries
                .into_iter()
                .find(|entry| entry.get_str("display_id") == Some(requested_entry))
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Archive.org item has no requested file {requested_entry}"),
                    )
                })?;
            let mut merged = info;
            for (key, value) in selected.iter() {
                merged.insert(key, value.clone());
            }
            return Ok(ExtractorResult::single(merged));
        }
        if entries.len() == 1 {
            let selected = entries.pop().expect("one Archive.org entry");
            let mut merged = info;
            for (key, value) in selected.iter() {
                merged.insert(key, value.clone());
            }
            return Ok(ExtractorResult::single(merged));
        }
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn google_drive_mime_extension(mime_type: Option<&str>) -> Option<&'static str> {
    match mime_type {
        Some("video/mp4") => Some("mp4"),
        Some("video/webm") => Some("webm"),
        Some("video/ogg") => Some("ogv"),
        Some("audio/mpeg") => Some("mp3"),
        Some("audio/mp4") => Some("m4a"),
        Some("audio/webm") => Some("webm"),
        Some("audio/ogg") => Some("ogg"),
        Some("audio/flac") => Some("flac"),
        _ => None,
    }
}

fn google_drive_filename(content_disposition: Option<&str>) -> Option<String> {
    let matcher = Regex::new(r#"(?i)\bfilename\s*=\s*(?:["']([^"']+)["']|([^;\s]+))"#).ok()?;
    let captures = matcher.captures(content_disposition?).ok().flatten()?;
    captures
        .get(1)
        .or_else(|| captures.get(2))
        .map(|value| value.as_str().to_owned())
}

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

/// Native Bandcamp track extractor. Track metadata and playable encodings are
/// read from the page's tralbum/embed JSON attributes without executing the
/// Bandcamp player.
pub struct BandcampTrackExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BandcampTrackExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BandcampTrackExtractor {
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
                "Bandcamp track URL did not match its native pattern",
            )
        })?;
        let uploader = captures
            .name("uploader")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Bandcamp URL has no uploader",
                )
            })?;
        let page_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Bandcamp URL has no track slug",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let tralbum = html_data_json_attribute(&html, "tralbum").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Bandcamp page has no tralbum JSON",
            )
        })?;
        let track_info = tralbum
            .get("trackinfo")
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Bandcamp page has no track information",
                )
            })?;
        let mut formats = Vec::new();
        if let Some(files) = track_info
            .get("file")
            .and_then(serde_json::Value::as_object)
        {
            for (format_id, value) in files {
                let Some(raw_url) = value.as_str() else {
                    continue;
                };
                let Some((extension, bitrate)) = format_id.split_once('-') else {
                    continue;
                };
                let media_url = raw_url
                    .strip_prefix("//")
                    .map_or_else(|| raw_url.to_owned(), |url| format!("https://{url}"));
                let mut format = serde_json::json!({
                    "format_id": format_id,
                    "url": media_url,
                    "ext": extension,
                    "vcodec": "none",
                    "acodec": extension,
                });
                if let Ok(bitrate) = bitrate.parse::<i64>() {
                    format["abr"] = serde_json::json!(bitrate);
                }
                formats.push(format);
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Bandcamp track {page_id} has no playable encodings"),
            ));
        }
        let embed = html_data_json_attribute(&html, "embed").unwrap_or(serde_json::Value::Null);
        let current = tralbum.get("current").unwrap_or(&serde_json::Value::Null);
        let track = json_string(track_info, "title").map(str::to_owned);
        let artist = json_string(&embed, "artist")
            .or_else(|| json_string(current, "artist"))
            .or_else(|| json_string(&tralbum, "artist"))
            .map(str::to_owned);
        let title = match (artist.as_deref(), track.as_deref()) {
            (Some(artist), Some(track)) => format!("{artist} - {track}"),
            (None, Some(track)) => track.to_owned(),
            (_, None) => page_id.to_owned(),
        };
        let track_id =
            json_value_string(track_info.get("track_id").or_else(|| track_info.get("id")))
                .or_else(|| json_value_string(tralbum.get("id")))
                .unwrap_or_else(|| page_id.to_owned());
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id.clone()));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp3")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("track", track);
        info.insert_if_some("artist", artist.clone());
        info.insert_if_some("uploader", artist);
        info.insert("uploader_id", serde_json::json!(uploader));
        info.insert(
            "uploader_url",
            serde_json::json!(format!("https://{uploader}.bandcamp.com")),
        );
        info.insert_if_some("album", json_string(&embed, "album_title"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some("duration", json_f64(track_info, "duration"));
        info.insert_if_some("track_number", json_i64(track_info, "track_num"));
        info.insert("track_id", serde_json::json!(track_id));
        if let Ok(tag_matcher) = Regex::new(
            r#"(?is)<(?:a|span)\b[^>]*class\s*=\s*["'][^"']*\btag\b[^"']*["'][^>]*>(.*?)</(?:a|span)>"#,
        ) {
            let tags = tag_matcher
                .captures_iter(&html)
                .flatten()
                .filter_map(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                })
                .filter(|tag| !tag.is_empty())
                .map(serde_json::Value::String)
                .collect::<Vec<_>>();
            if !tags.is_empty() {
                info.insert("tags", serde_json::Value::Array(tags));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

const BANNED_VIDEO_QUERY: &str = r#"
query GetVideoAndComments($id: String!) {
    getVideo(id: $id) {
        streamUrl
        directUrl
        unlisted
        live
        tags { name }
        title
        summary
        playCount
        largeImage
        videoDuration
        channel { _id title }
        createdAt
    }
    getVideoComments(id: $id, limit: 999999, offset: 0) {
        _id
        content
        user { _id username }
        voteCount { positive }
        createdAt
        replyCount
    }
}"#;

const BANNED_COMMENT_REPLIES_QUERY: &str = r#"
query GetCommentReplies($id: String!) {
    getCommentReplies(id: $id, limit: 999999, offset: 0) {
        _id
        content
        user { _id username }
        voteCount { positive }
        createdAt
        replyCount
    }
}"#;

fn banned_video_call(
    context: &ExtractionContext,
    id: &str,
    operation: &str,
    query: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let response = native_post_json(
        context,
        "https://api.infowarsmedia.com/graphql",
        &serde_json::json!({
            "variables": {"id": id},
            "operationName": operation,
            "query": query,
        }),
    )?;
    response.get("data").cloned().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "BannedVideo GraphQL response has no data",
        )
    })
}

fn banned_comment_value(comment: &serde_json::Value, parent: &str) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    value.insert(
        "id".to_owned(),
        json_value_string(comment.get("_id"))
            .map_or(serde_json::Value::Null, serde_json::Value::String),
    );
    value.insert(
        "text".to_owned(),
        json_string(comment, "content")
            .map_or(serde_json::Value::Null, |text| serde_json::json!(text)),
    );
    if let Some(user) = comment.get("user") {
        if let Some(username) = json_string(user, "username") {
            value.insert("author".to_owned(), serde_json::json!(username));
        }
        if let Some(user_id) = json_value_string(user.get("_id")) {
            value.insert("author_id".to_owned(), serde_json::json!(user_id));
        }
    }
    if let Some(timestamp) = comment.get("createdAt") {
        value.insert("timestamp".to_owned(), timestamp.clone());
    }
    value.insert("parent".to_owned(), serde_json::json!(parent));
    if let Some(likes) = comment
        .get("voteCount")
        .and_then(|votes| json_i64(votes, "positive"))
    {
        value.insert("like_count".to_owned(), serde_json::json!(likes));
    }
    serde_json::Value::Object(value)
}

/// Native BannedVideo GraphQL extractor. Metadata, media variants, and
/// available comments are fetched with typed Rust requests and no scripting
/// runtime.
pub struct BannedVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BannedVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BannedVideoExtractor {
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
                "BannedVideo URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "BannedVideo URL has no ID")
            })?;
        let data = banned_video_call(context, video_id, "GetVideoAndComments", BANNED_VIDEO_QUERY)?;
        let video = data.get("getVideo").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "BannedVideo response has no video object",
            )
        })?;
        let mut formats = Vec::new();
        if let Some(media_url) = json_string(video, "directUrl") {
            formats.push(serde_json::json!({
                "format_id": "direct",
                "quality": 1,
                "url": media_url,
                "ext": "mp4",
                "protocol": "http",
            }));
        }
        if let Some(media_url) = json_string(video, "streamUrl") {
            formats.push(serde_json::json!({
                "format_id": "hls",
                "url": media_url,
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "BannedVideo response has no playable media URLs",
            ));
        }
        let title = json_string(video, "title")
            .map(|title| title.strip_suffix('.').unwrap_or(title).to_owned())
            .unwrap_or_else(|| video_id.to_owned());
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some(
            "is_live",
            video.get("live").and_then(serde_json::Value::as_bool),
        );
        info.insert_if_some("description", json_string(video, "summary"));
        if let Some(channel) = video.get("channel") {
            info.insert_if_some("channel", json_string(channel, "title"));
            info.insert_if_some("channel_id", json_value_string(channel.get("_id")));
        }
        info.insert_if_some("view_count", json_i64(video, "playCount"));
        info.insert_if_some("thumbnail", json_string(video, "largeImage"));
        info.insert_if_some("duration", json_f64(video, "videoDuration"));
        if let Some(tags) = video.get("tags").and_then(serde_json::Value::as_array) {
            let tags = tags
                .iter()
                .filter_map(|tag| json_string(tag, "name"))
                .map(|tag| serde_json::json!(tag))
                .collect::<Vec<_>>();
            info.insert("tags", serde_json::Value::Array(tags));
        }
        if let Some(comments) = data
            .get("getVideoComments")
            .and_then(serde_json::Value::as_array)
        {
            let mut all_comments = Vec::new();
            for comment in comments {
                let comment_id = json_value_string(comment.get("_id")).unwrap_or_default();
                all_comments.push(banned_comment_value(comment, "root"));
                if json_i64(comment, "replyCount").unwrap_or_default() > 0 && !comment_id.is_empty()
                {
                    if let Ok(reply_data) = banned_video_call(
                        context,
                        &comment_id,
                        "GetCommentReplies",
                        BANNED_COMMENT_REPLIES_QUERY,
                    ) {
                        if let Some(replies) = reply_data
                            .get("getCommentReplies")
                            .and_then(serde_json::Value::as_array)
                        {
                            all_comments.extend(
                                replies
                                    .iter()
                                    .map(|reply| banned_comment_value(reply, &comment_id)),
                            );
                        }
                    }
                }
            }
            info.insert("comments", serde_json::Value::Array(all_comments));
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Coub API extractor. All media variants and counters are read from
/// the Coub JSON response and represented as ordinary Rust format records.
pub struct CoubExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CoubExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CoubExtractor {
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
                "Coub URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Coub URL has no ID")
            })?;
        let coub = context.get_json(&format!("http://coub.com/api/v2/coubs/{video_id}.json"))?;
        if let Some(error) = json_string(&coub, "error") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("Coub API error: {error}"),
            ));
        }
        let file_versions = coub
            .get("file_versions")
            .and_then(serde_json::Value::as_object);
        let mut formats = Vec::new();
        if let Some(html5) = file_versions
            .and_then(|versions| versions.get("html5"))
            .and_then(serde_json::Value::as_object)
        {
            for (kind, media_type) in [("video", "video"), ("audio", "audio")] {
                let Some(qualities) = html5.get(kind).and_then(serde_json::Value::as_object) else {
                    continue;
                };
                for (quality, item) in qualities {
                    let Some(media_url) = item.get("url").and_then(serde_json::Value::as_str)
                    else {
                        continue;
                    };
                    let default_ext = if media_type == "audio" { "mp3" } else { "mp4" };
                    let ext = yt_dlp_core::determine_ext(Some(media_url), default_ext);
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": format!("html5-{media_type}-{quality}"),
                        "ext": ext,
                        "quality": match quality.as_str() {
                            "low" => 0,
                            "med" => 1,
                            "high" => 2,
                            "higher" => 3,
                            _ => -1,
                        },
                        "vcodec": if media_type == "audio" { "none" } else { "unknown" },
                        "acodec": if media_type == "video" { "none" } else { "unknown" },
                    });
                    if let Some(size) = json_i64(item, "size") {
                        format["filesize"] = serde_json::json!(size);
                    }
                    formats.push(format);
                }
            }
        }
        if let Some(item) = file_versions
            .and_then(|versions| versions.get("iphone"))
            .and_then(serde_json::Value::as_object)
        {
            if let Some(media_url) = json_string(&serde_json::Value::Object(item.clone()), "url") {
                formats.push(serde_json::json!({
                    "url": media_url,
                    "format_id": "iphone",
                    "ext": yt_dlp_core::determine_ext(Some(media_url), "mp4"),
                }));
            }
        }
        if let Some(media_url) = file_versions
            .and_then(|versions| versions.get("mobile"))
            .and_then(|mobile| json_string(mobile, "audio_url"))
        {
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "mobile-audio",
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mp3"),
                "vcodec": "none",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Coub API returned no playable formats for {video_id}"),
            ));
        }
        let channel = coub.get("channel");
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&coub, "title"));
        info.insert_if_some("thumbnail", json_string(&coub, "picture"));
        info.insert_if_some("duration", json_f64(&coub, "duration"));
        info.insert_if_some(
            "timestamp",
            json_string(&coub, "published_at")
                .or_else(|| json_string(&coub, "created_at"))
                .and_then(yt_dlp_core::parse_iso8601),
        );
        info.insert_if_some(
            "uploader",
            channel.and_then(|value| json_string(value, "title")),
        );
        info.insert_if_some(
            "uploader_id",
            channel.and_then(|value| json_string(value, "permalink")),
        );
        info.insert_if_some(
            "view_count",
            json_i64(&coub, "views_count").or_else(|| json_i64(&coub, "views_increase_count")),
        );
        info.insert_if_some("like_count", json_i64(&coub, "likes_count"));
        info.insert_if_some("repost_count", json_i64(&coub, "recoubs_count"));
        if let Some(age_restricted) = json_bool(&coub, "age_restricted")
            .or_else(|| json_bool(&coub, "age_restricted_by_admin"))
        {
            info.insert(
                "age_limit",
                serde_json::json!(if age_restricted { 18 } else { 0 }),
            );
        }
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
        Ok(ExtractorResult::single(info))
    }
}

/// Native Vocaroo direct-audio extractor. The media host is selected from the
/// ID shape and a Rust HEAD request preserves the upload timestamp header.
pub struct VocarooExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl VocarooExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for VocarooExtractor {
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
                "Vocaroo URL did not match its native pattern",
            )
        })?;
        let audio_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Vocaroo URL has no ID")
            })?;
        let media_subdomain =
            if audio_id.len() == 10 || (audio_id.len() == 12 && audio_id.starts_with('1')) {
                "media1"
            } else {
                "media"
            };
        let media_url = format!("https://{media_subdomain}.vocaroo.com/mp3/{audio_id}");
        let mut request = Request::new(&media_url);
        request.set_method("HEAD").map_err(map_request_error)?;
        request.headers_mut().set("Referer", "https://vocaroo.com/");
        let response = context.request(&request)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(""));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert(
            "http_headers",
            serde_json::json!({"Referer": "https://vocaroo.com/"}),
        );
        if let Some(timestamp) = response
            .headers()
            .get("x-bz-upload-timestamp")
            .and_then(|value| value.parse::<f64>().ok())
            .map(|value| value / 1000.0)
        {
            info.insert("timestamp", serde_json::json!(timestamp));
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Freesound HTML/Open Graph extractor. The page metadata is enough to
/// build the same low/high audio format set without browser execution.
pub struct FreesoundExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FreesoundExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FreesoundExtractor {
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
                "Freesound URL did not match its native pattern",
            )
        })?;
        let audio_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Freesound URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let audio_url = html_meta_value(&html, "og:audio").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Freesound page {audio_id} has no audio URL"),
            )
        })?;
        let audio_url = audio_url
            .strip_prefix("https://freesound.org")
            .filter(|value| value.starts_with("http"))
            .unwrap_or(&audio_url)
            .to_owned();
        let mut audio_urls = vec![audio_url.clone()];
        if audio_url.contains("-lq.mp3") {
            audio_urls.push(audio_url.replace("-lq.mp3", "-hq.mp3"));
        }
        let channels = Regex::new(r#"(?is)Channels\s*</dt>\s*<dd[^>]*>(.*?)</dd>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| {
                captures
                    .get(1)
                    .map(|value| html_text_fragment(value.as_str()))
            });
        let formats = audio_urls
            .into_iter()
            .enumerate()
            .map(|(quality, media_url)| {
                serde_json::json!({
                    "url": media_url,
                    "format_id": if quality == 0 { "lq" } else { "hq" },
                    "ext": "mp3",
                    "format_note": channels.as_deref(),
                    "quality": quality,
                    "vcodec": "none",
                })
            })
            .collect::<Vec<_>>();
        let duration =
            Regex::new(r#"(?is)class\s*=\s*["'][^"']*\bduration\b[^"']*["'][^>]*>([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| value.as_str().trim().to_owned())
                })
                .and_then(|value| {
                    value
                        .parse::<f64>()
                        .map(|value| value / 1000.0)
                        .ok()
                        .or_else(|| yt_dlp_core::parse_duration(&value))
                });
        let description =
            Regex::new(r#"(?is)\bid\s*=\s*["']sound_description["'][^>]*>(.*?)</div>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                });
        let tags = Regex::new(r#"(?is)<a\b[^>]*>([^<]+)</a>"#)
            .ok()
            .and_then(|matcher| {
                let container = Regex::new(
                    r#"(?is)class\s*=\s*["'][^"']*\btags\b[^"']*["'][^>]*>(.*?)</(?:div|section)>"#,
                )
                .ok()?;
                let captures = container.captures(&html).ok().flatten()?;
                let body = captures.get(1)?.as_str();
                let values = matcher
                    .captures_iter(body)
                    .flatten()
                    .filter_map(|captures| {
                        captures
                            .get(1)
                            .map(|value| html_text_fragment(value.as_str()))
                    })
                    .filter(|tag| !tag.is_empty())
                    .map(serde_json::Value::String)
                    .collect::<Vec<_>>();
                (!values.is_empty()).then_some(serde_json::Value::Array(values))
            });
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:audio:title").or_else(|| html_meta_value(&html, "og:title")),
        );
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("uploader", html_meta_value(&html, "og:audio:artist"));
        info.insert_if_some("tags", tags);
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Yandex Disk extractor. The page store, public download URL, and
/// server-provided video streams are consumed directly by Rust.
pub struct YandexDiskExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl YandexDiskExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for YandexDiskExtractor {
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
                "Yandex Disk URL did not match its native pattern",
            )
        })?;
        let mut video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Yandex Disk URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let store = html_script_json(&html, "store-prefetch")?;
        let resource_id = json_value_string(store.get("rootResourceId")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Yandex Disk store has no root resource ID",
            )
        })?;
        let resource = store
            .get("resources")
            .and_then(serde_json::Value::as_object)
            .and_then(|resources| resources.get(&resource_id))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Yandex Disk store has no root resource",
                )
            })?;
        let title = json_string(resource, "name")
            .map(str::to_owned)
            .unwrap_or_else(|| video_id.clone());
        if let Some(public_key) = resource
            .get("meta")
            .and_then(|meta| json_string(meta, "short_url"))
        {
            if let Some(public_id) = self
                .matcher
                .captures(public_key)
                .ok()
                .flatten()
                .and_then(|captures| captures.name("id"))
                .map(|value| value.as_str().to_owned())
            {
                video_id = public_id;
            }
        }
        let meta = resource.get("meta").unwrap_or(&serde_json::Value::Null);
        let mut formats = Vec::new();
        let mut source_request =
            Request::new("https://cloud-api.yandex.net/v1/disk/public/resources/download");
        source_request.update_query(&[("public_key".to_owned(), url.to_owned())]);
        if let Ok(source) = context.request(&source_request) {
            if let Ok(source_json) = serde_json::from_slice::<serde_json::Value>(source.body()) {
                if let Some(source_url) = json_string(&source_json, "href") {
                    let ext = yt_dlp_core::determine_ext(
                        Some(&title),
                        json_string(meta, "ext")
                            .or_else(|| json_string(meta, "mime_type"))
                            .unwrap_or("mp4"),
                    );
                    formats.push(serde_json::json!({
                        "url": source_url,
                        "format_id": "source",
                        "ext": ext,
                        "quality": 1,
                        "filesize": json_i64(meta, "size"),
                    }));
                }
            }
        }
        if let Some(video_streams) = resource.get("videoStreams") {
            if let Some(videos) = video_streams
                .get("videos")
                .and_then(serde_json::Value::as_array)
            {
                for video in videos {
                    let Some(stream_url) = json_string(video, "url") else {
                        continue;
                    };
                    let size = video.get("size");
                    let height = json_i64(size.unwrap_or(&serde_json::Value::Null), "height");
                    let format_id =
                        height.map_or_else(|| "hls".to_owned(), |height| format!("hls-{height}p"));
                    formats.push(serde_json::json!({
                        "url": stream_url,
                        "format_id": format_id,
                        "ext": "mp4",
                        "height": height,
                        "width": json_i64(size.unwrap_or(&serde_json::Value::Null), "width"),
                        "protocol": "m3u8_native",
                    }));
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Yandex Disk resource {video_id} has no native media formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
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
        if let Some(duration) = video_streams_duration(resource) {
            info.insert("duration", serde_json::json!(duration));
        }
        if let Some(uid) = json_string(resource, "uid") {
            info.insert("uploader_id", serde_json::json!(uid));
            if let Some(display_name) = store
                .get("users")
                .and_then(serde_json::Value::as_object)
                .and_then(|users| users.get(uid))
                .and_then(|user| json_string(user, "displayName"))
            {
                info.insert("uploader", serde_json::json!(display_name));
            }
        }
        info.insert_if_some("view_count", json_i64(meta, "views_counter"));
        Ok(ExtractorResult::single(info))
    }
}

fn video_streams_duration(resource: &serde_json::Value) -> Option<f64> {
    resource
        .get("videoStreams")
        .and_then(|streams| json_f64(streams, "duration"))
        .map(|duration| duration / 1000.0)
}

/// Native Rumble embed API extractor. The embed JSON exposes direct, audio,
/// HLS, captions, live state, and author metadata without executing its player.
pub struct RumbleEmbedExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RumbleEmbedExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RumbleEmbedExtractor {
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
                "Rumble embed URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Rumble embed URL has no ID")
            })?;
        let mut request = Request::new("https://rumble.com/embedJS/u3/");
        request.update_query(&[
            ("request".to_owned(), "video".to_owned()),
            ("ver".to_owned(), "2".to_owned()),
            ("v".to_owned(), video_id.to_owned()),
        ]);
        let video = context.get_json(request.url())?;
        let live_status = match (
            json_i64(&video, "live"),
            json_bool(&video, "livestream_has_dvr"),
        ) {
            (Some(0), Some(true)) => "was_live",
            (Some(0), _) => "not_live",
            (Some(1), Some(false)) => "was_live",
            (Some(1), _) => "is_upcoming",
            (Some(2), _) => "is_live",
            _ => "",
        };
        let mut formats = Vec::new();
        if let Some(format_groups) = video.get("ua").and_then(serde_json::Value::as_object) {
            for (format_type, format_info) in format_groups {
                let candidates = match format_info {
                    serde_json::Value::Array(values) => {
                        values.iter().map(|value| (None, value)).collect::<Vec<_>>()
                    }
                    serde_json::Value::Object(values) => values
                        .iter()
                        .map(|(height, value)| (Some(height.as_str()), value))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                for (height_hint, video_info) in candidates {
                    let Some(media_url) = json_string(video_info, "url") else {
                        continue;
                    };
                    if format_type == "tar" {
                        continue;
                    }
                    let meta = video_info.get("meta").unwrap_or(&serde_json::Value::Null);
                    let height = json_i64(meta, "h")
                        .or_else(|| height_hint.and_then(|height| height.parse::<i64>().ok()));
                    if format_type == "hls" {
                        formats.push(serde_json::json!({
                            "url": media_url,
                            "format_id": "hls",
                            "ext": "mp4",
                            "protocol": "m3u8_native",
                        }));
                        continue;
                    }
                    let is_timeline = format_type == "timeline";
                    let is_audio = format_type == "audio";
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": height.map_or_else(
                            || format_type.to_owned(),
                            |height| format!("{format_type}-{height}p")
                        ),
                        "format_note": if is_timeline { "Timeline" } else { "" },
                        "vcodec": if is_audio { "none" } else { "unknown" },
                        "acodec": if is_timeline { "none" } else { "unknown" },
                        "fps": if is_timeline || is_audio {
                            serde_json::Value::Null
                        } else {
                            video.get("fps").cloned().unwrap_or(serde_json::Value::Null)
                        },
                    });
                    for (source, target) in [
                        ("bitrate", "tbr"),
                        ("size", "filesize"),
                        ("w", "width"),
                        ("h", "height"),
                    ] {
                        if let Some(value) = meta.get(source) {
                            format[target] = value.clone();
                        }
                    }
                    formats.push(format);
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Rumble video {video_id} has no playable formats"),
            ));
        }
        let author = video.get("author").unwrap_or(&serde_json::Value::Null);
        let mut info = InfoDict::new();
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            json_string(&video, "title").map(unescape_html_attribute),
        );
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
        info.insert_if_some(
            "timestamp",
            json_string(&video, "pubDate").and_then(yt_dlp_core::parse_iso8601),
        );
        info.insert_if_some("channel", json_string(author, "name"));
        info.insert_if_some("channel_url", json_string(author, "url"));
        info.insert_if_some("uploader", json_string(author, "name"));
        if !live_status.is_empty() {
            info.insert("live_status", serde_json::json!(live_status));
        }
        if live_status != "is_live" && live_status != "post_live" {
            info.insert_if_some("duration", json_i64(&video, "duration"));
        }
        let mut thumbnails = Vec::new();
        if let Some(values) = video.get("t").and_then(serde_json::Value::as_array) {
            thumbnails.extend(values.iter().filter_map(|thumbnail| {
                let url = json_string(thumbnail, "i")?;
                Some(serde_json::json!({
                    "url": url,
                    "width": json_i64(thumbnail, "w"),
                    "height": json_i64(thumbnail, "h"),
                }))
            }));
        }
        if thumbnails.is_empty() {
            if let Some(thumbnail) = json_string(&video, "i") {
                thumbnails.push(serde_json::json!({"url": thumbnail}));
            }
        }
        if !thumbnails.is_empty() {
            info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        }
        if let Some(captions) = video.get("cc").and_then(serde_json::Value::as_object) {
            let subtitles = captions
                .iter()
                .filter_map(|(language, caption)| {
                    let path = json_string(caption, "path")?;
                    Some((
                        language.clone(),
                        serde_json::json!([{
                            "url": path,
                            "name": json_string(caption, "language").unwrap_or("")
                        }]),
                    ))
                })
                .collect::<serde_json::Map<_, _>>();
            if !subtitles.is_empty() {
                info.insert("subtitles", serde_json::Value::Object(subtitles));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Clyp API extractor. The API response already contains stable media
/// URLs, so this port does not depend on browser JavaScript or an embedded
/// interpreter.
pub struct ClypExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ClypExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ClypExtractor {
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
        let audio_id = last_path_segment(url)?;
        let mut api_request = Request::new(format!("https://api.clyp.it/{audio_id}"));
        if let Ok(parsed) = url::Url::parse(url) {
            if let Some(token) = parsed
                .query_pairs()
                .find(|(name, _)| name == "token")
                .map(|(_, value)| value.into_owned())
            {
                api_request.update_query(&[("token".to_owned(), token)]);
            }
        }
        let metadata = context.get_json(api_request.url())?;
        let mut formats = Vec::new();
        for secure in ["", "Secure"] {
            for extension in ["Ogg", "Mp3"] {
                let key = format!("{secure}{extension}Url");
                let Some(format_url) = json_string(&metadata, &key) else {
                    continue;
                };
                formats.push(serde_json::json!({
                    "url": format_url,
                    "format_id": format!("{secure}{extension}"),
                    "ext": extension.to_ascii_lowercase(),
                    "vcodec": "none",
                    "acodec": extension.to_ascii_lowercase(),
                }));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Clyp API returned no playable formats for {audio_id}"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert(
            "title",
            serde_json::json!(json_string(&metadata, "Title").unwrap_or(&audio_id)),
        );
        info.insert_if_some("description", json_string(&metadata, "Description"));
        info.insert_if_some("duration", json_f64(&metadata, "Duration"));
        info.insert("formats", serde_json::Value::Array(formats));
        if let Some(value) = first.get("url") {
            info.insert("url", value.clone());
        }
        if let Some(value) = first.get("ext") {
            info.insert("ext", value.clone());
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Breitbart extractor. Breitbart exposes a JWPlayer HLS manifest whose
/// URL is derived from the video ID; page metadata is read with the native HTTP
/// stack and the existing Rust HLS downloader handles the media.
pub struct BreitbartExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BreitbartExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Audius extractor. Host discovery, URL resolution, and stream URL
/// construction are performed through the Rust request context; the service's
/// JavaScript frontend is not needed.
pub struct AudiusExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudiusExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Blerp GraphQL extractor. The query is intentionally limited to the
/// fields needed for a downloadable audio result, which keeps the Rust port
/// deterministic and avoids the web application's JavaScript bundle.
pub struct BlerpExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BlerpExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Acast episode extractor. Acast exposes episode metadata through a
/// small JSON endpoint, so the Rust port can preserve the audio result without
/// scraping or executing the embed player.
pub struct AcastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AcastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Acast show/playlist extractor. Playlist entry construction is fully
/// native; selecting and downloading entries is kept as an explicit CLI TODO
/// until the playlist scheduler is ported.
pub struct AcastChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AcastChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Dumpert JSON extractor. Media variants are represented as ordinary
/// Rust format records; HLS variants are handed to the native HLS downloader
/// by URL detection in the CLI.
pub struct DumpertExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DumpertExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Audiodraft entry extractor for contest URLs that already expose the
/// numeric entry ID. The custom-domain page-discovery variant remains an
/// explicit TODO because it requires a second HTML player parser.
pub struct AudiodraftExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudiodraftExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Audiomack song extractor. The song endpoint provides a final media
/// URL and canonical metadata; wrapper URLs for another service are surfaced
/// as TODO instead of being delegated to a different runtime.
pub struct AudiomackExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudiomackExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Aitube.kz extractor. The page's Next.js data and the service's HLS
/// endpoint are both consumed directly by Rust.
pub struct AitubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AitubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AitubeExtractor {
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
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Aitube URL: {error}"),
            )
        })?;
        let video_id = parsed
            .query_pairs()
            .find(|(name, _)| name == "id")
            .map(|(_, value)| value.into_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Aitube URL has no id query")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let next_data = html_script_json(&html, "__NEXT_DATA__")?;
        let video_info = next_data
            .get("props")
            .and_then(|props| props.get("pageProps"))
            .and_then(|page_props| page_props.get("videoInfo"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Aitube page has no videoInfo data",
                )
            })?;
        let hls_url = format!(
            "https://api-http.aitube.kz/kz.aitudala.aitube.staticaccess/video/{video_id}/video"
        );
        let fallback_title = html_meta_value(&html, "og:title");
        let title = json_string(video_info, "title")
            .or(fallback_title.as_deref())
            .unwrap_or(&video_id)
            .to_owned();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(hls_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": hls_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        info.insert_if_some("description", json_string(video_info, "description"));
        for (source, target) in [
            ("viewCount", "view_count"),
            ("likeCount", "like_count"),
            ("commentCount", "comment_count"),
            ("channelSubscriberCount", "channel_follower_count"),
        ] {
            if let Some(value) = video_info.get(source) {
                info.insert(target, value.clone());
            }
        }
        for (source, target) in [
            ("channelTitle", "channel"),
            ("channelId", "channel_id"),
            ("coverUrl", "thumbnail"),
        ] {
            info.insert_if_some(target, json_string(video_info, source));
        }
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for AudiomackExtractor {
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
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Audiomack URL: {error}"),
            )
        })?;
        let path = parsed.path().trim_matches('/');
        let song_tag = path
            .split_once("song/")
            .map(|(_, tag)| tag)
            .filter(|tag| !tag.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Audiomack URL has no song path",
                )
            })?;
        let mut request = Request::new(format!(
            "http://www.audiomack.com/api/music/url/song/{song_tag}"
        ));
        request.update_query(&[("extended".to_owned(), "1".to_owned())]);
        let response = context.get_json(request.url())?;
        let media_url = json_string(&response, "url")
            .filter(|url| !url.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Audiomack API returned no song URL",
                )
            })?;
        if media_url.contains("soundcloud.com/") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                "TODO: native SoundCloud wrapper extraction is not implemented",
            ));
        }
        let ext = yt_dlp_core::determine_ext(Some(media_url), "mp3");
        let id = json_value_string(response.get("id")).unwrap_or_else(|| {
            media_url
                .rsplit('/')
                .next()
                .unwrap_or(song_tag)
                .split('?')
                .next()
                .unwrap_or(song_tag)
                .trim_end_matches(&format!(".{ext}"))
                .to_owned()
        });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(id));
        info.insert_if_some("uploader", json_string(&response, "artist"));
        info.insert_if_some("title", json_string(&response, "title"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for AudiodraftExtractor {
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
                "Audiodraft URL did not match its native pattern",
            )
        })?;
        let entry_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Audiodraft URL has no ID")
            })?;
        let mut request =
            Request::new("https://www.audiodraft.com/scripts/general/player/getPlayerInfoNew.php");
        request.set_method("POST").map_err(map_request_error)?;
        request.headers_mut().set(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        );
        request
            .headers_mut()
            .set("X-Requested-With", "XMLHttpRequest");
        request.set_data(Some(format!("id=player_entry_{entry_id}").into_bytes()));
        let response = context.request(&request)?;
        let data: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Audiodraft response: {error}"),
            )
        })?;
        let media_url = json_string(&data, "path").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Audiodraft response has no media path",
            )
        })?;
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(
                json_value_string(data.get("entry_id")).unwrap_or_else(|| entry_id.to_owned())
            ),
        );
        info.insert_if_some("title", json_string(&data, "entry_title"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "mp3",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some("uploader", json_string(&data, "designer_name"));
        info.insert_if_some("uploader_id", json_value_string(data.get("designer_id")));
        info.insert_if_some("webpage_url", json_string(&data, "entry_url"));
        info.insert_if_some("like_count", json_i64(&data, "entry_likes"));
        info.insert_if_some("average_rating", json_i64(&data, "entry_rating"));
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for DumpertExtractor {
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
                "Dumpert URL did not match its native pattern",
            )
        })?;
        let normalized_id = captures
            .name("id")
            .map(|value| value.as_str().replace('_', "/"))
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Dumpert URL has no ID")
            })?;
        let api_id = normalized_id.replace('/', "_");
        let response = context.get_json(&format!(
            "http://api-live.dumpert.nl/mobile_api/json/info/{api_id}"
        ))?;
        let item = response
            .get("items")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Dumpert API returned no item",
                )
            })?;
        let media = item
            .get("media")
            .and_then(serde_json::Value::as_array)
            .and_then(|media| {
                media.iter().find(|media| {
                    media.get("mediatype").and_then(serde_json::Value::as_str) == Some("VIDEO")
                })
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Dumpert item has no VIDEO media",
                )
            })?;
        let formats = media
            .get("variants")
            .and_then(serde_json::Value::as_array)
            .map(|variants| {
                variants
                    .iter()
                    .filter_map(|variant| {
                        let url = variant.get("uri").and_then(serde_json::Value::as_str)?;
                        let version = variant
                            .get("version")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("source");
                        let detected_ext = yt_dlp_core::determine_ext(Some(url), "mp4");
                        let ext = if detected_ext == "m3u8" {
                            "mp4".to_owned()
                        } else {
                            detected_ext
                        };
                        Some(serde_json::json!({
                            "url": url,
                            "format_id": version,
                            "ext": ext,
                            "protocol": if url.split('?').next().is_some_and(|url| url.ends_with(".m3u8")) {
                                "m3u8_native"
                            } else {
                                "http"
                            },
                        }))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Dumpert media has no playable variants",
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(normalized_id));
        info.insert_if_some("title", json_string(item, "title"));
        info.insert_if_some("description", json_string(item, "description"));
        info.insert_if_some(
            "duration",
            media.get("duration").and_then(serde_json::Value::as_f64),
        );
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or(serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        if let Some(stills) = item.get("stills").and_then(serde_json::Value::as_object) {
            let thumbnails = stills
                .iter()
                .filter_map(|(id, value)| {
                    value
                        .as_str()
                        .map(|url| serde_json::json!({"id": id, "url": url}))
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        if let Some(stats) = item.get("stats") {
            info.insert_if_some(
                "like_count",
                stats.get("kudos_total").and_then(|value| value.as_i64()),
            );
            info.insert_if_some(
                "view_count",
                stats.get("views_total").and_then(|value| value.as_i64()),
            );
        }
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for AcastChannelExtractor {
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
                "Acast channel URL did not match its native pattern",
            )
        })?;
        let show_slug = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Acast show has no ID")
            })?;
        let show = context.get_json(&format!(
            "https://feeder.acast.com/api/v1/shows/{show_slug}"
        ))?;
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(json_string(&show, "id").unwrap_or(show_slug)),
        );
        info.insert_if_some("title", json_string(&show, "title"));
        info.insert_if_some("description", json_string(&show, "description"));
        let show_info = show
            .as_object()
            .map(|show| {
                serde_json::json!({
                    "creator": show.get("author"),
                    "series": show.get("title"),
                })
            })
            .unwrap_or(serde_json::Value::Null);
        let entries = show
            .get("episodes")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Acast show response has no episodes array",
                )
            })?
            .iter()
            .filter_map(|episode| {
                let media_url = json_string(episode, "url")?;
                let episode_id =
                    json_string(episode, "id").or_else(|| json_string(episode, "episodeUrl"))?;
                let title = json_string(episode, "title").unwrap_or(episode_id);
                let ext = yt_dlp_core::determine_ext(Some(media_url), "mp3");
                let mut entry = InfoDict::new();
                entry.insert("id", serde_json::json!(episode_id));
                entry.insert("title", serde_json::json!(title));
                entry.insert("url", serde_json::json!(media_url));
                entry.insert("ext", serde_json::json!(ext.clone()));
                entry.insert(
                    "formats",
                    serde_json::json!([{
                        "url": media_url,
                        "format_id": "audio",
                        "ext": ext,
                        "vcodec": "none",
                    }]),
                );
                entry.insert_if_some("description", json_string(episode, "description"));
                entry.insert_if_some("thumbnail", json_string(episode, "image"));
                if let Some(value) = episode.get("duration").and_then(|value| value.as_f64()) {
                    entry.insert("duration", serde_json::json!(value));
                }
                if let Some(value) = show_info.get("creator").and_then(|value| value.as_str()) {
                    entry.insert("creator", serde_json::json!(value));
                }
                if let Some(value) = show_info.get("series").and_then(|value| value.as_str()) {
                    entry.insert("series", serde_json::json!(value));
                }
                Some(entry)
            })
            .collect::<Vec<_>>();
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

impl InfoExtractor for AcastExtractor {
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
                "Acast URL did not match its native pattern",
            )
        })?;
        let channel = captures
            .name("channel")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Acast URL has no channel")
            })?;
        let display_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Acast URL has no episode ID",
                )
            })?;
        let mut api_request = Request::new(format!(
            "https://feeder.acast.com/api/v1/shows/{channel}/episodes/{display_id}"
        ));
        api_request.update_query(&[("showInfo".to_owned(), "true".to_owned())]);
        let episode = context.get_json(api_request.url())?;
        let episode_url = json_string(&episode, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Acast episode has no media URL",
            )
        })?;
        let ext = yt_dlp_core::determine_ext(Some(episode_url), "mp3");
        let title = json_string(&episode, "title")
            .map(str::to_owned)
            .unwrap_or_else(|| display_id.to_owned());
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(json_string(&episode, "id").unwrap_or(display_id)),
        );
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title.clone()));
        info.insert("episode", serde_json::json!(title));
        info.insert("url", serde_json::json!(episode_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": episode_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        info.insert_if_some("description", json_string(&episode, "description"));
        info.insert_if_some("thumbnail", json_string(&episode, "image"));
        info.insert_if_some("duration", json_f64(&episode, "duration"));
        info.insert_if_some("filesize", json_f64(&episode, "contentLength"));
        if let Some(show) = episode.get("show") {
            info.insert_if_some("creator", json_string(show, "author"));
            info.insert_if_some("series", json_string(show, "title"));
        }
        for (source, target) in [("season", "season_number"), ("episode", "episode_number")] {
            if let Some(value) = episode.get(source).and_then(|value| value.as_i64()) {
                info.insert(target, serde_json::json!(value));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for BlerpExtractor {
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
        let audio_id = last_path_segment(url)?;
        let payload = serde_json::json!({
            "operationName": "webBitePageGetBite",
            "variables": {"_id": audio_id},
            "query": "query webBitePageGetBite($_id: MongoID!) { web { biteById(_id: $_id) { _id title userKeywords ownerObject { _id username } audio { mp3 { url } } } } }",
        });
        let mut request = Request::new("https://api.blerp.com/graphql");
        request.set_method("POST").map_err(map_request_error)?;
        request
            .headers_mut()
            .set("Content-Type", "application/json");
        request.set_data(Some(serde_json::to_vec(&payload).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("could not encode Blerp GraphQL request: {error}"),
            )
        })?));
        let response = context.request(&request)?;
        let response: serde_json::Value =
            serde_json::from_slice(response.body()).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Blerp GraphQL response: {error}"),
                )
            })?;
        let bite = response
            .get("data")
            .and_then(|data| data.get("web"))
            .and_then(|web| web.get("biteById"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Blerp GraphQL response has no bite",
                )
            })?;
        let media_url = bite
            .get("audio")
            .and_then(|audio| audio.get("mp3"))
            .and_then(|mp3| mp3.get("url"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Blerp response has no MP3 URL",
                )
            })?;
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(json_string(bite, "_id").unwrap_or(&audio_id)),
        );
        info.insert(
            "title",
            serde_json::json!(json_string(bite, "title").unwrap_or(&audio_id)),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "mp3",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        if let Some(owner) = bite.get("ownerObject") {
            info.insert_if_some("uploader", json_string(owner, "username"));
            info.insert_if_some("uploader_id", json_string(owner, "_id"));
        }
        if let Some(tags) = bite
            .get("userKeywords")
            .and_then(serde_json::Value::as_array)
        {
            info.insert("tags", serde_json::Value::Array(tags.clone()));
        }
        Ok(ExtractorResult::single(info))
    }
}

fn audius_data<'a>(
    response: &'a serde_json::Value,
) -> Result<&'a serde_json::Value, ExtractorError> {
    response.get("data").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Audius API response has no data field",
        )
    })
}

fn json_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .unwrap_or_else(|| value.to_string())
    })
}

impl InfoExtractor for AudiusExtractor {
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
        let hosts_response = context.get_json("https://api.audius.co/")?;
        let hosts = audius_data(&hosts_response)?;
        let host = hosts
            .as_array()
            .and_then(|hosts| hosts.iter().find_map(|host| host.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Audius host discovery returned no API hosts",
                )
            })?
            .trim_end_matches('/')
            .to_owned();
        let track_response = if self.descriptor.key == "AudiusTrackIE" {
            let track_id = url
                .rsplit('/')
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::InvalidUrl,
                        "Audius track URL has no track ID",
                    )
                })?;
            context.get_json(&format!("{host}/v1/tracks/{track_id}"))?
        } else {
            let mut resolve_request = Request::new(format!("{host}/v1/resolve"));
            resolve_request.update_query(&[("url".to_owned(), url.to_owned())]);
            context.get_json(resolve_request.url())?
        };
        let track_data = audius_data(&track_response)?;
        let track_id = json_value_string(track_data.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Audius response has no track ID",
            )
        })?;
        let title = json_string(track_data, "title")
            .map(str::to_owned)
            .unwrap_or_else(|| track_id.clone());
        let stream_url = format!("{host}/v1/tracks/{track_id}/stream");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id));
        info.insert("title", serde_json::json!(title.clone()));
        info.insert("track", serde_json::json!(title));
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "stream",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some("description", json_string(track_data, "description"));
        info.insert_if_some("duration", json_f64(track_data, "duration"));
        info.insert_if_some("genre", json_string(track_data, "genre"));
        for (name, source) in [
            ("view_count", "play_count"),
            ("like_count", "favorite_count"),
            ("repost_count", "repost_count"),
        ] {
            if let Some(value) = track_data.get(source) {
                info.insert(name, value.clone());
            }
        }
        if let Some(artist) = track_data
            .get("user")
            .and_then(|user| user.get("name"))
            .and_then(serde_json::Value::as_str)
        {
            info.insert("artist", serde_json::json!(artist));
        }
        if let Some(artwork) = track_data
            .get("artwork")
            .and_then(serde_json::Value::as_object)
        {
            let thumbnails = artwork
                .iter()
                .filter_map(|(quality, value)| {
                    value.as_str().map(|url| {
                        serde_json::json!({
                            "id": quality,
                            "url": url,
                        })
                    })
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for BreitbartExtractor {
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
        let video_id = path_segment_after(url, "v")?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let manifest_url = format!("https://cdn.jwplayer.com/manifests/{video_id}.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title").unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("url", serde_json::json!(manifest_url));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": manifest_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
