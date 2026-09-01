/// Native BerufeTV metadata/player API extractor.
pub struct BerufeTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BerufeTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BerufeTvExtractor {
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
        _url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matcher
            .captures(_url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "BerufeTV URL has no film ID",
                )
            })?;
        let metadata = berufetv_metadata(context, &video_id).unwrap_or_else(|_| serde_json::json!({}));
        let meta = metadata
            .get("metadaten")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| {
                items.iter().find(|item| {
                    json_string(item, "miId") == Some(video_id.as_str())
                })
            });
        let player_url = format!(
            "https://d.video-cdn.net/play/player/8YRzUk6pTzmBdrsLe9Y88W/video/{video_id}"
        );
        let video = context.get_json(&player_url)?;
        let sources = video
            .get("videoSources")
            .and_then(|value| value.get("html"))
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("BerufeTV video {video_id} has no HTML source map"),
                )
            })?;
        let mut formats = Vec::new();
        let mut source_entries = Vec::with_capacity(sources.len());
        if let Some(source_values) = sources.get("auto") {
            source_entries.push(("auto", source_values));
        }
        for (format_id, source_values) in sources {
            if format_id != "auto" {
                source_entries.push((format_id.as_str(), source_values));
            }
        }
        for (format_id, source_values) in source_entries {
            let Some(source) = source_values
                .as_array()
                .and_then(|values| values.first())
            else {
                continue;
            };
            let Some(source_url) = json_string(source, "source").filter(|value| !value.is_empty())
            else {
                continue;
            };
            if format_id == "auto" {
                formats.push(serde_json::json!({
                    "url": source_url,
                    "format_id": "hls",
                    "protocol": "m3u8_native",
                    "ext": "mp4",
                }));
                continue;
            }
            let extension = json_string(source, "mimeType")
                .and_then(|mime| mimetype_extension(Some(mime)))
                .unwrap_or_else(|| {
                    yt_dlp_core::determine_ext(Some(source_url), "unknown").to_ascii_lowercase()
                });
            if !matches!(extension.as_str(), "mp4" | "webm" | "ogv" | "mp3" | "m4a" | "ogg") {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: BerufeTV native extractor does not implement source type {}",
                        json_string(source, "mimeType").unwrap_or("unknown")
                    ),
                ));
            }
            formats.push(serde_json::json!({
                "url": source_url,
                "format_id": format_id,
                "protocol": "http",
                "ext": extension,
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("BerufeTV video {video_id} has no playable sources"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut subtitles = serde_json::Map::new();
        if let Some(tracks) = video.get("videoTracks").and_then(serde_json::Value::as_array) {
            for track in tracks {
                if json_string(track, "type") != Some("SUBTITLES") {
                    continue;
                }
                let Some(language) = json_string(track, "language") else {
                    continue;
                };
                let Some(subtitle_url) = json_string(track, "source") else {
                    continue;
                };
                subtitles.insert(
                    language.to_owned(),
                    serde_json::json!([{
                        "url": subtitle_url,
                        "name": json_string(track, "label"),
                        "ext": "vtt",
                    }]),
                );
            }
        }
        let title = meta
            .and_then(|meta| json_string(meta, "titel"))
            .or_else(|| {
                video
                    .get("videoMetaData")
                    .and_then(|metadata| json_string(metadata, "title"))
            })
            .map(html_text_fragment)
            .filter(|value| !value.is_empty());
        let description = meta
            .and_then(|meta| json_string(meta, "beschreibung"))
            .map(html_text_fragment)
            .filter(|value| !value.is_empty());
        let thumbnail = meta
            .and_then(|meta| json_string(meta, "thumbnail"))
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "https://asset-out-cdn.video-cdn.net/private/videos/{video_id}/thumbnails/active"
                )
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert("thumbnail", serde_json::json!(thumbnail));
        info.insert_if_some(
            "duration",
            json_f64(&video, "duration").map(|duration| duration / 1000.0),
        );
        info.insert_if_some(
            "categories",
            meta.and_then(|meta| json_string(meta, "kategorie"))
                .map(|category| vec![category]),
        );
        if let Some(tags) = meta.and_then(|meta| meta.get("themengebiete")) {
            info.insert("tags", tags.clone());
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
        info.insert("subtitles", serde_json::Value::Object(subtitles));
        Ok(ExtractorResult::single(info))
    }
}

fn berufetv_metadata(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(
        "https://rest.arbeitsagentur.de/infosysbub/berufetv/pc/v1/film-metadata",
    );
    request
        .headers_mut()
        .set("X-API-Key", "79089773-4892-4386-86e6-e8503669f426");
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid BerufeTV metadata for {video_id}: {error}"),
        )
    })
}
