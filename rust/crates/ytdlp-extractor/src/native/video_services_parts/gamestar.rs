/// Native GameStar/GamePro JSON-LD video extractor.
pub struct GameStarExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GameStarExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GameStarExtractor {
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
                "GameStar URL did not match its native pattern",
            )
        })?;
        let site = captures
            .name("site")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GameStar URL has no site")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "GameStar URL has no ID")
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let json_ld = gamestar_json_ld(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("GameStar video {video_id} has no VideoObject JSON-LD"),
            )
        })?;
        let raw_title = json_string(&json_ld, "name")
            .map(html_text_fragment)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("GameStar video {video_id} has no title"),
                )
            })?;
        let site_name = if site == "pro" { "GamePro" } else { "GameStar" };
        let title = raw_title
            .strip_suffix(&format!(" - {site_name}"))
            .unwrap_or(&raw_title)
            .to_owned();
        let media_url = format!(
            "http://gamestar.de/_misc/videos/portal/getVideoUrl.cfm?premium=0&videoId={video_id}"
        );
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::Value::Array(vec![serde_json::json!({
                "url": media_url,
                "format_id": "http",
                "protocol": "http",
                "ext": "mp4",
            })]),
        );
        info.insert_if_some(
            "description",
            json_string(&json_ld, "description").map(html_text_fragment),
        );
        info.insert_if_some(
            "thumbnail",
            gamestar_thumbnail(&json_ld),
        );
        info.insert_if_some(
            "duration",
            json_f64(&json_ld, "duration").or_else(|| {
                json_string(&json_ld, "duration")
                    .and_then(gamestar_duration)
            }),
        );
        info.insert_if_some(
            "timestamp",
            json_string(&json_ld, "uploadDate")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("view_count", json_i64(&json_ld, "interactionCount"));
        info.insert_if_some("comment_count", gamestar_comment_count(&webpage));
        Ok(ExtractorResult::single(info))
    }
}

fn gamestar_json_ld(html: &str) -> Option<serde_json::Value> {
    let matcher = Regex::new(
        r#"(?is)<script\b[^>]*\btype\s*=\s*["']application/ld\+json["'][^>]*>(.*?)</script>"#,
    )
    .ok()?;
    matcher.captures_iter(html).flatten().find_map(|captures| {
        let value = captures
            .get(1)
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value.as_str().trim()).ok())?;
        gamestar_video_object(&value)
    })
}

fn gamestar_video_object(value: &serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::Array(values) => values.iter().find_map(gamestar_video_object),
        serde_json::Value::Object(values) => {
            let is_video = values
                .get("@type")
                .and_then(|value| value.as_str())
                .is_some_and(|value| value == "VideoObject");
            if is_video {
                return Some(value.clone());
            }
            values.get("@graph").and_then(gamestar_video_object)
        }
        _ => None,
    }
}

fn gamestar_thumbnail(data: &serde_json::Value) -> Option<String> {
    let value = data
        .get("thumbnailUrl")
        .or_else(|| data.get("thumbnailURL"))
        .or_else(|| data.get("thumbnail_url"))?;
    match value {
        serde_json::Value::String(value)
            if value.starts_with("http://") || value.starts_with("https://") =>
        {
            Some(value.clone())
        }
        serde_json::Value::Array(values) => values.iter().find_map(|value| {
            value.as_str().filter(|value| {
                value.starts_with("http://") || value.starts_with("https://")
            }).map(str::to_owned)
        }),
        _ => None,
    }
}

fn gamestar_duration(value: &str) -> Option<f64> {
    yt_dlp_core::parse_duration(value).or_else(|| {
        let matcher = Regex::new(
            r#"^P(?:[0-9]+D)?T(?:(?P<hours>[0-9]+)H)?(?:(?P<minutes>[0-9]+)M)?(?:(?P<seconds>[0-9]+(?:\.[0-9]+)?)S)?$"#,
        )
        .ok()?;
        let captures = matcher.captures(value).ok().flatten()?;
        let hours = captures
            .name("hours")
            .and_then(|value| value.as_str().parse::<f64>().ok())
            .unwrap_or_default();
        let minutes = captures
            .name("minutes")
            .and_then(|value| value.as_str().parse::<f64>().ok())
            .unwrap_or_default();
        let seconds = captures
            .name("seconds")
            .and_then(|value| value.as_str().parse::<f64>().ok())
            .unwrap_or_default();
        Some(hours * 3_600.0 + minutes * 60.0 + seconds)
    })
}

fn gamestar_comment_count(html: &str) -> Option<i64> {
    let matcher = Regex::new(
        r#"(?is)<span>\s*Kommentare\s*</span>\s*<span[^>]*class\s*=\s*["'][^"']*\bcount\b[^"']*["'][^>]*>\s*\(\s*([0-9]+)"#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i64>().ok())
}
