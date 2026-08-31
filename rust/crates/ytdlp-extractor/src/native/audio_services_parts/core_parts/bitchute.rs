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
