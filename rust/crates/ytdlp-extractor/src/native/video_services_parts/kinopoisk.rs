/// Native KinoPoisk film playback extractor.
pub struct KinopoiskExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KinopoiskExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KinopoiskExtractor {
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
        let film_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "KinoPoisk URL has no film ID",
                )
            })?;
        let widget_url = kinopoisk_query_url(
            "https://ott-widget.kinopoisk.ru/v1/kp/",
            &[("kpId", film_id.as_str())],
        )?;
        let response = context.get(&widget_url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let models = kinopoisk_json_script(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KinoPoisk film {film_id} has no widget data"),
            )
        })?;
        let models = models.get("models").unwrap_or(&models);
        let film = models.get("filmStatus").unwrap_or(models);
        let title = json_string(film, "title")
            .or_else(|| json_string(film, "originalTitle"))
            .map(str::to_owned)
            .unwrap_or_else(|| film_id.clone());
        let playlist_url = models
            .get("playlistEntity")
            .and_then(|playlist| json_string(playlist, "uri"))
            .and_then(kinopoisk_valid_url)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("KinoPoisk film {film_id} has no playback manifest"),
                )
            })?;
        let formats = vec![serde_json::json!({
            "url": playlist_url,
            "format_id": "hls",
            "ext": "mp4",
            "protocol": "m3u8_native",
        })];
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let description = [
            "descriptscription",
            "description",
            "shortDescriptscription",
            "shortDescription",
        ]
        .iter()
        .find_map(|key| json_string(film, key))
        .map(str::to_owned);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(film_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some(
            "thumbnail",
            json_string(film, "coverUrl")
                .or_else(|| json_string(film, "posterUrl"))
                .map(str::to_owned),
        );
        info.insert_if_some("duration", json_i64(film, "duration"));
        info.insert_if_some("age_limit", json_i64(film, "restrictionAge"));
        info.insert(
            "url",
            first
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn kinopoisk_query_url(
    base: &str,
    fields: &[(&str, &str)],
) -> Result<String, ExtractorError> {
    let mut parsed = url::Url::parse(base).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid KinoPoisk endpoint {base}: {error}"),
        )
    })?;
    {
        let mut query = parsed.query_pairs_mut();
        for (key, value) in fields {
            query.append_pair(key, value);
        }
    }
    Ok(parsed.to_string())
}

fn kinopoisk_json_script(html: &str) -> Option<serde_json::Value> {
    let matcher = Regex::new(
        r#"(?is)<script\b[^>]*\btype\s*=\s*["']application/json["'][^>]*>(.*?)</script\s*>"#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| serde_json::from_str(value.as_str().trim()).ok())
}

fn kinopoisk_valid_url(value: &str) -> Option<String> {
    let value = value.trim();
    (value.starts_with("http://") || value.starts_with("https://"))
        .then(|| value.to_owned())
}
