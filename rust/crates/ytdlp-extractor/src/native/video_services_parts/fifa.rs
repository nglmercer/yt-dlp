/// Native FIFA+ page/API/HLS extractor.
///
/// FIFA exposes the player bootstrap through the page's preconnect origin,
/// then returns an Uplynk HLS URL from the preplay response.  The native
/// downloader consumes that manifest directly; no Python player or manifest
/// helper is involved.
pub struct FifaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FifaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FifaExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "FIFA+ URL has no video ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let preconnect_link = fifa_preconnect_link(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FIFA+ video {video_id} has no preconnect API origin"),
            )
        })?;
        let api_origin = preconnect_link.trim_end_matches('/');
        let details_url = format!("{api_origin}/sections/videoDetails/{video_id}");
        // The source extractor deliberately treats this endpoint as
        // non-fatal: the player data is still sufficient to produce a video.
        let video_details = context
            .get_json(&details_url)
            .unwrap_or_else(|_| serde_json::json!({}));

        let preplay_url = format!("{api_origin}/videoPlayerData/{video_id}");
        let preplay = context.get_json(&preplay_url)?;
        let parameters = preplay
            .get("preplayParameters")
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FIFA+ video {video_id} has no preplay parameters"),
                )
            })?;
        let content_id = fifa_required_string(parameters, "contentId", &video_id)?;
        let query_string = fifa_required_string(parameters, "queryStr", &video_id)?;
        let signature = fifa_required_string(parameters, "signature", &video_id)?;
        let content_url = format!(
            "https://content.uplynk.com/preplay/{content_id}/multiple.json?{query_string}&sig={signature}"
        );
        let content_data = context.get_json(&content_url)?;
        let play_url = content_data
            .get("playURL")
            .and_then(serde_json::Value::as_str)
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FIFA+ video {video_id} has no playable HLS URL"),
                )
            })?;

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&video_details, "title"));
        info.insert_if_some("description", json_string(&video_details, "description"));
        info.insert_if_some("duration", json_i64(&video_details, "duration"));
        info.insert_if_some(
            "release_timestamp",
            json_string(&video_details, "dateOfRelease")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        if let Some(categories) = fifa_categories(&video_details) {
            info.insert("categories", categories);
        }
        info.insert_if_some(
            "thumbnail",
            video_details
                .get("backgroundImage")
                .and_then(|image| json_string(image, "src")),
        );
        info.insert("url", serde_json::json!(play_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": play_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn fifa_preconnect_link(html: &str) -> Option<String> {
    let patterns = [
        r#"(?is)<link\b[^>]*\brel\s*=\s*[\"']preconnect[\"'][^>]*\bhref\s*=\s*[\"']([^\"']+)"#,
        r#"(?is)<link\b[^>]*\bhref\s*=\s*[\"']([^\"']+)[\"'][^>]*\brel\s*=\s*[\"']preconnect[\"']"#,
    ];
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().trim()))
            .map(|value| proto_relative_url(value, "https:"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
    })
}

fn fifa_required_string(
    value: &serde_json::Value,
    key: &str,
    video_id: &str,
) -> Result<String, ExtractorError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FIFA+ video {video_id} has no preplay {key}"),
            )
        })
}

fn fifa_categories(value: &serde_json::Value) -> Option<serde_json::Value> {
    let mut categories = Vec::new();
    for key in ["videoCategory", "videoSubcategory"] {
        match value.get(key) {
            Some(serde_json::Value::String(category)) if !category.is_empty() => {
                categories.push(category.clone());
            }
            Some(serde_json::Value::Array(values)) => categories.extend(
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .filter(|category| !category.is_empty())
                    .map(str::to_owned),
            ),
            _ => {}
        }
    }
    (!categories.is_empty()).then(|| serde_json::json!(categories))
}
