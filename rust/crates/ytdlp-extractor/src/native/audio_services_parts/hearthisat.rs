/// Native HearThisAt track/API extractor.
pub struct HearThisAtExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HearThisAtExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HearThisAtExtractor {
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
                "HearThisAt URL did not match its native pattern",
            )
        })?;
        let artist = captures
            .name("artist")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "HearThisAt URL has no artist")
            })?;
        let track_slug = captures
            .name("title")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "HearThisAt URL has no track title",
                )
            })?;
        let display_id = format!("{artist} - {track_slug}");
        let api_url = url
            .replace("www.", "")
            .replace("hearthis.at", "api-v2.hearthis.at");
        let data = context.get_json(&api_url)?;
        let track_id = json_value_string(data.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HearThisAt track {display_id} has no ID"),
            )
        })?;
        let username = data
            .get("user")
            .and_then(|user| json_string(user, "username"))
            .unwrap_or(&artist);
        let title = json_string(&data, "title")
            .map(|title| format!("{username} - {title}"))
            .unwrap_or_else(|| display_id.clone());
        let mut formats = Vec::new();
        if let Some(stream_url) = json_string(&data, "stream_url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            formats.push(serde_json::json!({
                "format_id": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
                "url": stream_url,
                "ext": "mp3",
            }));
        }
        if let (Some(download_url), Some(filename)) = (
            json_string(&data, "download_url"),
            json_string(&data, "download_filename"),
        ) {
            let extension = yt_dlp_core::determine_ext(Some(filename), "unknown").to_ascii_lowercase();
            if hearthis_at_known_extension(&extension)
                && (download_url.starts_with("http://") || download_url.starts_with("https://"))
            {
                formats.push(serde_json::json!({
                    "format_id": extension,
                    "vcodec": "none",
                    "acodec": extension,
                    "url": download_url,
                    "ext": extension,
                    "quality": 2,
                }));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HearThisAt track {track_id} has no playable audio formats"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", first_format.get("url").cloned().unwrap_or_default());
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp3")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("thumbnail", json_string(&data, "artwork_url").or_else(|| {
            json_string(&data, "thumb")
        }));
        info.insert_if_some("description", json_string(&data, "description"));
        info.insert_if_some("duration", hearthis_at_integer(data.get("duration")));
        info.insert_if_some("timestamp", hearthis_at_timestamp(data.get("release_timestamp")));
        info.insert_if_some("view_count", hearthis_at_integer(data.get("playback_count")));
        info.insert_if_some("genre", json_string(&data, "genre"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn hearthis_at_known_extension(extension: &str) -> bool {
    matches!(
        extension,
        "aac" | "flac" | "m4a" | "mp3" | "oga" | "ogg" | "opus" | "wav" | "webm"
    )
}

fn hearthis_at_integer(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        serde_json::Value::String(value) => value
            .replace([',', ' '], "")
            .parse::<i64>()
            .ok(),
        _ => None,
    }
}

fn hearthis_at_timestamp(value: Option<&serde_json::Value>) -> Option<i64> {
    hearthis_at_integer(value).or_else(|| {
        value
            .and_then(serde_json::Value::as_str)
            .and_then(|value| parse_timestamp(value.to_owned()))
    })
}
