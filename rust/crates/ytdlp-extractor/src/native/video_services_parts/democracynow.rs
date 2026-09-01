/// Native Democracy Now! page JSON/media extractor.
pub struct DemocracynowExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DemocracynowExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DemocracynowExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Democracy Now! URL has no display ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let json_data = json_object_after_marker(&webpage, "text/json").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Democracy Now! page {display_id} has no JSON media data"),
            )
        })?;
        let title = json_string(&json_data, "title")
            .filter(|value| !value.is_empty())
            .unwrap_or(&display_id)
            .to_owned();
        let mut formats = Vec::new();
        let mut video_id = None;
        for key in ["file", "audio", "video", "high_res_video"] {
            let Some(raw_url) = json_string(&json_data, key).filter(|value| !value.is_empty())
            else {
                continue;
            };
            let media_url = democracynow_media_url(url, raw_url);
            if video_id.is_none() {
                video_id = democracynow_media_id(&media_url);
            }
            let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
            let mut format = serde_json::Map::new();
            format.insert("url".to_owned(), serde_json::json!(media_url));
            format.insert("ext".to_owned(), serde_json::json!(extension));
            format.insert(
                "protocol".to_owned(),
                serde_json::json!(if extension == "m3u8" {
                    "m3u8_native"
                } else {
                    "http"
                }),
            );
            if key == "audio" {
                format.insert("vcodec".to_owned(), serde_json::json!("none"));
            }
            formats.push(serde_json::Value::Object(format));
        }
        let mut subtitles = serde_json::Map::new();
        if let Some(raw_url) = json_string(&json_data, "caption_file") {
            democracynow_add_subtitle(&mut subtitles, "en", url, raw_url);
        }
        if let Some(captions) = json_data.get("captions").and_then(serde_json::Value::as_array) {
            for caption in captions {
                let Some(raw_url) = json_string(caption, "url") else {
                    continue;
                };
                let language = json_string(caption, "language")
                    .filter(|value| !value.is_empty())
                    .unwrap_or("en")
                    .to_ascii_lowercase();
                democracynow_add_subtitle(&mut subtitles, &language, url, raw_url);
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Democracy Now! page {display_id} has no playable media"),
            ));
        }
        let video_id = video_id.unwrap_or(display_id);
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            html_meta_value(&webpage, "og:description")
                .or_else(|| html_meta_value(&webpage, "description"))
                .map(|value| html_text_fragment(&value)),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&json_data, "image").map(|value| resolve_url(url, value)),
        );
        info.insert("subtitles", serde_json::Value::Object(subtitles));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("url", first_format.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first_format.get("ext").and_then(serde_json::Value::as_str));
        Ok(ExtractorResult::single(info))
    }
}

fn democracynow_media_url(page_url: &str, raw_url: &str) -> String {
    let resolved = resolve_url(page_url, raw_url);
    let Ok(mut parsed) = url::Url::parse(&resolved) else {
        return resolved;
    };
    parsed.set_query(None);
    parsed.set_fragment(None);
    parsed.to_string()
}

fn democracynow_media_id(media_url: &str) -> Option<String> {
    let parsed = url::Url::parse(media_url).ok()?;
    let segment = parsed
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .next_back()?;
    let stem = segment.rsplit_once('.').map_or(segment, |(stem, _)| stem);
    Some(stem.strip_prefix("dn").unwrap_or(stem).to_owned())
}

fn democracynow_add_subtitle(
    subtitles: &mut serde_json::Map<String, serde_json::Value>,
    language: &str,
    page_url: &str,
    raw_url: &str,
) {
    let subtitle_url = resolve_url(page_url, raw_url);
    let entries = subtitles
        .entry(language.to_owned())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    if let serde_json::Value::Array(entries) = entries {
        entries.push(serde_json::json!({
            "url": subtitle_url,
            "ext": yt_dlp_core::determine_ext(Some(raw_url), "vtt"),
        }));
    }
}
