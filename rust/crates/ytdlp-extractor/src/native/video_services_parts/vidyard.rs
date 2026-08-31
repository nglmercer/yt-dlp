/// Native Vidyard player JSON extractor. The player endpoint exposes direct
/// media, HLS, captions, chapter metadata, and optional additional metadata;
/// multi-chapter players become native playlists.
pub struct VidyardExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

impl VidyardExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let mut matchers = Vec::new();
        for pattern in &descriptor.valid_urls {
            matchers.push(compile_source_pattern(pattern).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Vidyard URL pattern: {error}"),
                )
            })?);
        }
        Ok(Self {
            descriptor,
            matchers,
        })
    }
}

impl InfoExtractor for VidyardExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matchers
            .iter()
            .find_map(|matcher| {
                matcher
                    .captures(url)
                    .ok()
                    .flatten()
                    .and_then(|captures| captures.name("id"))
                    .map(|value| value.as_str().to_owned())
            })
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Vidyard URL has no ID")
            })?;
        let response =
            context.get_json(&format!("https://play.vidyard.com/player/{video_id}.json"))?;
        let payload = response.get("payload").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Vidyard player response has no payload",
            )
        })?;
        let chapters = payload
            .get("chapters")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Vidyard player payload has no chapters",
                )
            })?;
        let mut entries = Vec::new();
        for chapter in chapters {
            let mut entry = vidyard_chapter_info(chapter)?;
            if let Some(facade_id) = json_string(chapter, "facadeUuid") {
                if let Ok(additional) =
                    context.get_json(&format!("https://play.vidyard.com/video/{facade_id}"))
                {
                    merge_vidyard_additional_metadata(&mut entry, &additional);
                }
            }
            entries.push(entry);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Vidyard player {video_id} has no chapters"),
            ));
        }
        if entries.len() == 1 {
            return Ok(ExtractorResult::single(
                entries.pop().expect("one Vidyard chapter"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(
                json_string(payload, "playerUuid")
                    .or_else(|| json_string(payload, "playerUUID"))
                    .unwrap_or(&video_id)
            ),
        );
        info.insert_if_some("title", json_string(payload, "name"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn vidyard_chapter_info(chapter: &serde_json::Value) -> Result<InfoDict, ExtractorError> {
    let facade_id = json_string(chapter, "facadeUuid")
        .or_else(|| json_string(chapter, "id"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Vidyard chapter has no facadeUuid",
            )
        })?;
    let mut formats = Vec::new();
    let sources = chapter.get("sources").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Vidyard chapter has no sources",
        )
    })?;
    if let Some(hls) = sources.get("hls") {
        for source in json_object_values(hls) {
            let Some(media_url) = json_string(source, "url") else {
                continue;
            };
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        }
    }
    if let Some(sources) = sources.as_object() {
        for (source_type, source_list) in sources {
            if source_type == "hls" {
                continue;
            }
            for source in json_object_values(source_list) {
                let Some(media_url) = json_string(source, "url") else {
                    continue;
                };
                let profile = json_string(source, "profile");
                let mut format = serde_json::json!({
                    "url": media_url,
                    "format_id": format!("http-{source_type}{}", profile.map_or_else(String::new, |profile| format!("-{profile}"))),
                    "ext": mimetype_extension(json_string(source, "mimeType"))
                        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(media_url), "mp4")),
                });
                if let Some(profile) = profile {
                    if let Some((width, height)) = parse_resolution_label(profile) {
                        format["width"] = serde_json::json!(width);
                        format["height"] = serde_json::json!(height);
                    } else if let Some(height) = profile
                        .strip_suffix('p')
                        .and_then(|value| value.parse::<i64>().ok())
                    {
                        format["height"] = serde_json::json!(height);
                    }
                }
                formats.push(format);
            }
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Vidyard chapter {facade_id} has no playable sources"),
        ));
    }
    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(facade_id));
    info.insert_if_some(
        "display_id",
        json_i64(chapter, "videoId").map(|value| value.to_string()),
    );
    info.insert_if_some("title", json_string(chapter, "name"));
    info.insert_if_some(
        "description",
        json_string(chapter, "description").map(unescape_html_attribute),
    );
    info.insert_if_some(
        "duration",
        json_f64(chapter, "milliseconds")
            .map(|value| value / 1000.0)
            .or_else(|| json_f64(chapter, "seconds")),
    );
    if let Some(thumbnails) = chapter
        .get("thumbnailUrls")
        .and_then(serde_json::Value::as_object)
    {
        let values = thumbnails
            .values()
            .filter_map(|thumbnail| {
                let url = thumbnail
                    .as_str()
                    .or_else(|| json_string(thumbnail, "url"))?;
                Some(serde_json::json!({"url": url}))
            })
            .collect::<Vec<_>>();
        if !values.is_empty() {
            info.insert("thumbnails", serde_json::Value::Array(values));
        }
    }
    if let Some(captions) = chapter
        .get("captions")
        .and_then(serde_json::Value::as_array)
    {
        let mut subtitles = serde_json::Map::new();
        for caption in captions {
            let Some(url) = json_string(caption, "vttUrl") else {
                continue;
            };
            let language = json_string(caption, "language").unwrap_or("und");
            subtitles
                .entry(language.to_owned())
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .expect("subtitle value is an array")
                .push(serde_json::json!({
                    "url": url,
                    "name": json_string(caption, "name"),
                }));
        }
        if !subtitles.is_empty() {
            info.insert("subtitles", serde_json::Value::Object(subtitles));
        }
    }
    if let Some(tags) = chapter.get("tags").and_then(serde_json::Value::as_array) {
        info.insert(
            "tags",
            serde_json::Value::Array(
                tags.iter()
                    .filter_map(|tag| json_string(tag, "name"))
                    .map(|tag| serde_json::json!(tag))
                    .collect(),
            ),
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
    info.insert(
        "http_headers",
        serde_json::json!({"Referer": "https://play.vidyard.com/"}),
    );
    Ok(info)
}

fn merge_vidyard_additional_metadata(info: &mut InfoDict, metadata: &serde_json::Value) {
    info.insert_if_some(
        "title",
        json_string(metadata, "title").or_else(|| json_string(metadata, "name")),
    );
    info.insert_if_some("duration", json_f64(metadata, "seconds"));
    if let Some(thumbnails) = metadata
        .get("thumbnailUrl")
        .and_then(serde_json::Value::as_object)
        .and_then(|value| value.get("url"))
        .and_then(serde_json::Value::as_str)
    {
        info.insert("thumbnails", serde_json::json!([{"url": thumbnails}]));
    }
    if let Some(sections) = metadata
        .get("videoSections")
        .and_then(serde_json::Value::as_array)
    {
        let chapters = sections
            .iter()
            .filter_map(|section| {
                Some(serde_json::json!({
                    "title": json_string(section, "title")?,
                    "start_time": json_f64(section, "milliseconds").map(|value| value / 1000.0)?,
                }))
            })
            .collect::<Vec<_>>();
        if !chapters.is_empty() {
            info.insert("chapters", serde_json::Value::Array(chapters));
        }
    }
}
