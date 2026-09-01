/// Native JTBC VOD and program-replay extractors.
pub struct JtbcExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JtbcExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JtbcExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "JTBC URL has no episode ID")
            })?;
        let video_id = if display_id.starts_with("vo") {
            display_id.to_ascii_uppercase()
        } else {
            let response = context.get(url)?;
            let webpage = String::from_utf8_lossy(response.body());
            Regex::new(r#"(?is)\bdata-vod\s*=\s*["'](VO\d+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("JTBC episode {display_id} has no VOD ID"),
                    )
                })?
        };
        let playback = context.get_json(&format!("https://api.jtbc.co.kr/vod/{video_id}"))?;
        let mut subtitles = serde_json::Map::new();
        for track in playback
            .get("tracks")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(media_url) = json_string(track, "file") else {
                continue;
            };
            let language = json_string(track, "label").unwrap_or("und");
            subtitles
                .entry(language.to_owned())
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .expect("JTBC subtitle entry is an array")
                .push(serde_json::json!({"url": media_url}));
        }
        let mut formats = Vec::new();
        jtbc_collect_files(
            playback
                .get("sources")
                .and_then(|sources| sources.get("HLS")),
            &mut formats,
        );
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JTBC VOD {video_id} has no HLS sources"),
            ));
        }
        let metadata = context
            .get_json(&format!(
                "https://now-api.jtbc.co.kr/v1/vod/detail?vodFileId={video_id}"
            ))
            .ok()
            .unwrap_or(serde_json::Value::Null);
        let detail = metadata.get("vodDetail").unwrap_or(&metadata);
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(detail, "vodTitleView"));
        info.insert_if_some("series", json_string(detail, "programTitle"));
        info.insert_if_some("description", json_string(detail, "episodeContents"));
        info.insert_if_some("thumbnail", json_string(detail, "imgFileUrl"));
        info.insert_if_some("age_limit", json_i64(detail, "watchAge"));
        info.insert_if_some(
            "release_date",
            json_string(detail, "broadcastDate").and_then(jtbc_date_digits),
        );
        info.insert_if_some(
            "duration",
            json_value_string(playback.get("playTime"))
                .and_then(|value| yt_dlp_core::parse_duration(&value)),
        );
        info.insert(
            "url",
            first
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::Value::Object(subtitles));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native JTBC program replay playlist extractor.
pub struct JtbcProgramExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JtbcProgramExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JtbcProgramExtractor {
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
        let program_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "JTBC program URL has no ID")
            })?;
        let api_url = format!(
            "https://now-api.jtbc.co.kr/v1/vodClip/programHome/programReplayVodList?programId={program_id}&rowCount=10000"
        );
        let response = context.get_json(&api_url)?;
        let mut entries = Vec::new();
        jtbc_collect_episode_ids(&response, &mut entries);
        entries.sort();
        entries.dedup();
        let entries = entries
            .into_iter()
            .map(|video_id| {
                let mut entry =
                    native_url_result(&format!("https://vod.jtbc.co.kr/player/program/{video_id}"));
                entry.insert("ie_key", serde_json::json!("JTBC"));
                entry
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JTBC program {program_id} has no replay episodes"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(program_id));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn jtbc_collect_files(value: Option<&serde_json::Value>, formats: &mut Vec<serde_json::Value>) {
    match value {
        Some(serde_json::Value::Object(values)) => {
            if let Some(media_url) = values
                .get("file")
                .and_then(serde_json::Value::as_str)
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            {
                let normalized = Regex::new(r#"/playlist_pd\d+\.m3u8"#)
                    .ok()
                    .map(|matcher| matcher.replace(media_url, "/playlist.m3u8").into_owned())
                    .unwrap_or_else(|| media_url.to_owned());
                formats.push(serde_json::json!({
                    "url": normalized,
                    "format_id": "hls",
                    "ext": "mp4",
                    "protocol": "m3u8_native",
                }));
            }
            for child in values.values() {
                jtbc_collect_files(Some(child), formats);
            }
        }
        Some(serde_json::Value::Array(values)) => {
            for child in values {
                jtbc_collect_files(Some(child), formats);
            }
        }
        _ => {}
    }
}

fn jtbc_collect_episode_ids(value: &serde_json::Value, episode_ids: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(values) => {
            if let Some(video_id) = json_value_string(values.get("episodeId"))
                .filter(|value| !value.is_empty())
            {
                episode_ids.push(video_id);
            }
            for child in values.values() {
                jtbc_collect_episode_ids(child, episode_ids);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                jtbc_collect_episode_ids(child, episode_ids);
            }
        }
        _ => {}
    }
}

fn jtbc_date_digits(value: &str) -> Option<String> {
    let matcher = Regex::new(r#"(\d{4})\D?(\d{2})\D?(\d{2})"#).ok()?;
    let captures = matcher.captures(value).ok().flatten()?;
    Some(format!(
        "{}{}{}",
        captures.get(1)?.as_str(),
        captures.get(2)?.as_str(),
        captures.get(3)?.as_str()
    ))
}
