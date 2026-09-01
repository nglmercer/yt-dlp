/// Native 3Cat/TV3/Catalunya Ràdio media API extractor.
pub struct CcmaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CcmaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CcmaExtractor {
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
                "3Cat URL did not match its native pattern",
            )
        })?;
        let media_type = captures
            .name("type")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "3Cat URL has no media type")
            })?;
        let media_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "3Cat URL has no media ID")
            })?;
        let mut request = Request::new("http://api-media.3cat.cat/pvideo/media.jsp");
        request.update_query(&[
            ("media".to_owned(), media_type.clone()),
            ("idint".to_owned(), media_id.clone()),
            ("format".to_owned(), "dm".to_owned()),
        ]);
        let response = context.request(&request)?;
        let media: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid 3Cat media JSON for {media_id}: {error}"),
            )
        })?;
        let mut formats = Vec::new();
        let media_url = media
            .get("media")
            .and_then(|value| value.get("url"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("3Cat media {media_id} has no media URL"),
                )
            })?;
        if let Some(sources) = media_url.as_array() {
            for source in sources {
                ccma_add_format(&mut formats, source, &media_type, &media_id)?;
            }
        } else if let Some(source_url) = media_url.as_str() {
            ccma_add_direct_format(&mut formats, source_url, &media_type, None, &media_id)?;
        }
        let information = media.get("informacio").unwrap_or(&serde_json::Value::Null);
        let title = json_string(information, "titol")
            .filter(|value| !value.is_empty())
            .unwrap_or(&media_id)
            .to_owned();
        let duration_data = information.get("durada");
        let duration = duration_data
            .and_then(|value| json_i64(value, "milisegons"))
            .map(|value| value as f64 / 1000.0)
            .or_else(|| {
                duration_data
                    .and_then(|value| json_string(value, "text"))
                    .and_then(|value| yt_dlp_core::parse_duration(value))
            });
        let timestamp = information
            .get("data_emissio")
            .and_then(|value| json_string(value, "utc"))
            .and_then(|value| parse_timestamp(value.to_owned()));
        let subtitles = ccma_subtitles(media.get("subtitols"));
        let thumbnails = ccma_thumbnails(media.get("imatges"));
        let age_limit = information
            .get("codi_etic")
            .and_then(|value| json_string(value, "id"))
            .and_then(|value| value.split_once('_'))
            .and_then(|(_, value)| {
                if value == "TP" {
                    Some(0)
                } else {
                    value.parse::<i64>().ok()
                }
            });
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("3Cat media {media_id} has no playable formats"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(media_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            json_string(information, "descripcio").map(html_text_fragment),
        );
        info.insert_if_some("duration", duration);
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some(
            "upload_date",
            information
                .get("data_emissio")
                .and_then(|value| json_string(value, "utc"))
                .and_then(date_digits),
        );
        info.insert_if_some("thumbnails", thumbnails);
        info.insert("subtitles", subtitles);
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("age_limit", age_limit);
        info.insert_if_some("alt_title", json_string(information, "titol_complet"));
        info.insert_if_some("episode_number", json_i64(information, "capitol"));
        info.insert_if_some(
            "categories",
            information
                .get("tematica")
                .and_then(|value| json_string(value, "text"))
                .map(|value| vec![value]),
        );
        info.insert_if_some("series", json_string(information, "programa"));
        info.insert_if_some("url", first_format.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first_format.get("ext").and_then(serde_json::Value::as_str));
        Ok(ExtractorResult::single(info))
    }
}

fn ccma_add_format(
    formats: &mut Vec<serde_json::Value>,
    source: &serde_json::Value,
    media_type: &str,
    media_id: &str,
) -> Result<(), ExtractorError> {
    let Some(source_url) = json_string(source, "file").filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    ccma_add_direct_format(
        formats,
        source_url,
        media_type,
        json_string(source, "label"),
        media_id,
    )
}

fn ccma_add_direct_format(
    formats: &mut Vec<serde_json::Value>,
    source_url: &str,
    media_type: &str,
    label: Option<&str>,
    media_id: &str,
) -> Result<(), ExtractorError> {
    let extension = yt_dlp_core::determine_ext(Some(source_url), "unknown");
    if matches!(extension.as_str(), "f4m" | "smil")
        || source_url.starts_with("rtmp://")
        || source_url.starts_with("rtmps://")
    {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: 3Cat native extractor does not implement stream {source_url}"),
        ));
    }
    let mut format = serde_json::Map::new();
    format.insert("url".to_owned(), serde_json::json!(source_url));
    format.insert(
        "protocol".to_owned(),
        serde_json::json!(if extension == "mpd" {
            "http_dash_segments"
        } else if extension == "m3u8" {
            "m3u8_native"
        } else {
            "http"
        }),
    );
    if extension != "unknown" {
        format.insert("ext".to_owned(), serde_json::json!(extension));
    }
    if let Some(label) = label.filter(|value| !value.is_empty()) {
        format.insert("format_id".to_owned(), serde_json::json!(label));
        let (width, height) = ccma_resolution(label);
        if let Some(width) = width {
            format.insert("width".to_owned(), serde_json::json!(width));
        }
        if let Some(height) = height {
            format.insert("height".to_owned(), serde_json::json!(height));
        }
    }
    if media_type == "audio" {
        format.insert("vcodec".to_owned(), serde_json::json!("none"));
    }
    if extension == "mpd" {
        format.insert("format_id".to_owned(), serde_json::json!("dash"));
        format.insert("ext".to_owned(), serde_json::json!("mp4"));
    }
    if format.get("url").is_none() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("3Cat media {media_id} has an invalid source"),
        ));
    }
    formats.push(serde_json::Value::Object(format));
    Ok(())
}

fn ccma_resolution(label: &str) -> (Option<i64>, Option<i64>) {
    let Ok(matcher) = Regex::new(r"(?i)(?:(\d{3,4})x(\d{3,4})|(\d{3,4})p)") else {
        return (None, None);
    };
    let Some(captures) = matcher.captures(label).ok().flatten() else {
        return (None, None);
    };
    if let (Some(width), Some(height)) = (captures.get(1), captures.get(2)) {
        return (
            width.as_str().parse().ok(),
            height.as_str().parse().ok(),
        );
    }
    (None, captures.get(3).and_then(|value| value.as_str().parse().ok()))
}

fn ccma_subtitles(value: Option<&serde_json::Value>) -> serde_json::Value {
    let mut subtitles = serde_json::Map::new();
    let Some(value) = value else {
        return serde_json::Value::Object(subtitles);
    };
    let items = match value {
        serde_json::Value::Array(items) => items.iter().collect::<Vec<_>>(),
        serde_json::Value::Object(_) => vec![value],
        _ => Vec::new(),
    };
    for item in items {
        let Some(subtitle_url) = json_string(item, "url").filter(|value| !value.is_empty()) else {
            continue;
        };
        let language = json_string(item, "iso")
            .or_else(|| json_string(item, "text"))
            .unwrap_or("ca");
        let entries = subtitles
            .entry(language.to_owned())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let serde_json::Value::Array(entries) = entries {
            entries.push(serde_json::json!({"url": subtitle_url}));
        }
    }
    serde_json::Value::Object(subtitles)
}

fn ccma_thumbnails(value: Option<&serde_json::Value>) -> Option<Vec<serde_json::Value>> {
    let value = value?;
    let thumbnail_url = json_string(value, "url").filter(|value| !value.is_empty())?;
    let mut thumbnail = serde_json::Map::new();
    thumbnail.insert("url".to_owned(), serde_json::json!(thumbnail_url));
    if let Some(width) = json_i64(value, "amplada") {
        thumbnail.insert("width".to_owned(), serde_json::json!(width));
    }
    if let Some(height) = json_i64(value, "alcada") {
        thumbnail.insert("height".to_owned(), serde_json::json!(height));
    }
    Some(vec![serde_json::Value::Object(thumbnail)])
}
