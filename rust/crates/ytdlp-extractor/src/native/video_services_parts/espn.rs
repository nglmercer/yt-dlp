/// Native ESPN public clip API extractor.
pub struct EspnExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EspnExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EspnExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "ESPN URL has no clip ID")
            })?;
        let data = context.get_json(&format!(
            "http://api-app.espn.com/v1/video/clips/{video_id}"
        ))?;
        let clip = data
            .get("videos")
            .and_then(serde_json::Value::as_array)
            .and_then(|videos| videos.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ESPN clip {video_id} has no video record"),
                )
            })?;
        let title = json_string(clip, "headline")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ESPN clip {video_id} has no headline"),
                )
            })?;
        let mut formats = Vec::new();
        let mut seen_urls = Vec::new();
        for key in ["source", "mobile"] {
            if let Some(sources) = clip.get("links").and_then(|links| links.get(key)) {
                espn_collect_formats(
                    sources,
                    None,
                    &mut formats,
                    &mut seen_urls,
                    video_id.as_str(),
                )?;
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ESPN clip {video_id} has no playable source URLs"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            json_string(clip, "caption").or_else(|| json_string(clip, "description")),
        );
        info.insert_if_some("thumbnail", json_string(clip, "thumbnail"));
        info.insert_if_some("duration", json_i64(clip, "duration"));
        info.insert_if_some(
            "timestamp",
            json_string(clip, "originalPublishDate")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
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
        Ok(ExtractorResult::single(info))
    }
}

fn espn_collect_formats(
    sources: &serde_json::Value,
    base_source_id: Option<String>,
    formats: &mut Vec<serde_json::Value>,
    seen_urls: &mut Vec<String>,
    video_id: &str,
) -> Result<(), ExtractorError> {
    let Some(sources) = sources.as_object() else {
        return Ok(());
    };
    for (source_id, source) in sources {
        if source_id == "alert" {
            continue;
        }
        match source {
            serde_json::Value::String(source_url) => {
                espn_add_format(
                    source_url,
                    base_source_id.as_deref(),
                    formats,
                    seen_urls,
                    video_id,
                )?;
            }
            serde_json::Value::Object(_) => {
                let format_id = base_source_id
                    .as_deref()
                    .map(|base| format!("{base}-{source_id}"))
                    .unwrap_or_else(|| source_id.to_owned());
                espn_collect_formats(
                    source,
                    Some(format_id),
                    formats,
                    seen_urls,
                    video_id,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn espn_add_format(
    source_url: &str,
    source_id: Option<&str>,
    formats: &mut Vec<serde_json::Value>,
    seen_urls: &mut Vec<String>,
    video_id: &str,
) -> Result<(), ExtractorError> {
    if !source_url.starts_with("http://") && !source_url.starts_with("https://") {
        return Ok(());
    }
    if seen_urls.iter().any(|url| url == source_url) {
        return Ok(());
    }
    seen_urls.push(source_url.to_owned());
    let extension = yt_dlp_core::determine_ext(Some(source_url), "mp4");
    if matches!(extension.as_str(), "smil" | "f4m") {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: ESPN clip {video_id} requires unsupported {extension} manifest parsing"
            ),
        ));
    }
    let is_hls = extension.eq_ignore_ascii_case("m3u8");
    let mut format = serde_json::json!({
        "url": source_url,
        "format_id": source_id.unwrap_or(if is_hls { "hls" } else { "http" }),
        "protocol": if is_hls { "m3u8_native" } else { "http" },
        "ext": if is_hls { "mp4" } else { extension.as_str() },
    });
    if let Some(captures) = Regex::new(r"(?i)(\d+)p(\d+)_(\d+)k\.")
        .ok()
        .and_then(|matcher| matcher.captures(source_url).ok().flatten())
    {
        if let Some(height) = captures
            .get(1)
            .and_then(|value| value.as_str().parse::<i64>().ok())
        {
            format["height"] = serde_json::json!(height);
        }
        if let Some(fps) = captures
            .get(2)
            .and_then(|value| value.as_str().parse::<i64>().ok())
        {
            format["fps"] = serde_json::json!(fps);
        }
        if let Some(tbr) = captures
            .get(3)
            .and_then(|value| value.as_str().parse::<i64>().ok())
        {
            format["tbr"] = serde_json::json!(tbr);
        }
    }
    if source_id == Some("mezzanine") {
        format["quality"] = serde_json::json!(1);
    }
    formats.push(format);
    Ok(())
}
