/// Native FreeTV movie extractor.
pub struct FreeTvMoviesExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FreeTvMoviesExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FreeTvMoviesExtractor {
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
        let display_id = freetv_match_id(&self.matcher, url, "FreeTV movie")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let content_id = freetv_page_video_id(&webpage, &display_id)?;
        let data = freetv_api_response(
            context,
            &content_id,
            &[
                ("action", "olyott_video_play"),
                ("contentID", content_id.as_str()),
            ],
        )?;
        let display_meta = data.get("displayMeta").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FreeTV movie {content_id} has no display metadata"),
            )
        })?;
        let stream_url = json_string(display_meta, "streamURLVideo")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("FreeTV movie {content_id} has no HLS URL"),
                )
            })?;
        let formats = freetv_hls_formats(stream_url);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(content_id));
        info.insert_if_some("title", json_string(display_meta, "title"));
        info.insert_if_some("description", json_string(display_meta, "desc"));
        info.insert("url", serde_json::json!(stream_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

/// Native FreeTV series/season playlist extractor.
pub struct FreeTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FreeTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FreeTvExtractor {
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
        let playlist_id = freetv_match_id(&self.matcher, url, "FreeTV series")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let title = freetv_class_text(&webpage, "h1", "synopis");
        let description = freetv_class_text(&webpage, "div", "synopis content");
        let season_ids = Regex::new(r#"(?is)<option\b[^>]*\bvalue\s*=\s*["'](\d+)["']"#)
            .ok()
            .map(|matcher| {
                matcher
                    .captures_iter(&webpage)
                    .flatten()
                    .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut entries = Vec::new();
        for season_id in season_ids {
            let data = freetv_api_response(
                context,
                &season_id,
                &[
                    ("contentID", season_id.as_str()),
                    ("action", "olyott_get_dynamic_series_content"),
                    ("type", "list"),
                    ("perPage", "1000"),
                ],
            )?;
            let empty_episodes = Vec::new();
            let episodes = data
                .get("1")
                .and_then(serde_json::Value::as_array)
                .unwrap_or(&empty_episodes);
            for episode in episodes {
                if let Some(entry) = freetv_episode_entry(episode, title.as_deref()) {
                    entries.push(entry);
                }
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn freetv_api_response(
    context: &ExtractionContext,
    content_id: &str,
    parameters: &[(&str, &str)],
) -> Result<serde_json::Value, ExtractorError> {
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in parameters {
        form.append_pair(key, value);
    }
    let mut request =
        Request::new("https://www.freetv.com/wordpress/wp-admin/admin-ajax.php");
    request.set_method("POST").map_err(map_request_error)?;
    request
        .headers_mut()
        .set("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8");
    request.headers_mut().set("Accept", "application/json");
    request.set_data(Some(form.finish().into_bytes()));
    let response = context.request(&request)?;
    let root = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid FreeTV API response for {content_id}: {error}"),
        )
    })?;
    root.get("data").cloned().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("FreeTV API response for {content_id} has no data"),
        )
    })
}

fn freetv_page_video_id(
    webpage: &str,
    display_id: &str,
) -> Result<String, ExtractorError> {
    let patterns = [
        r#"(?is)\bclass\s*=\s*["'][^"']*\bpostid-(\d+)"#,
        r#"(?is)<link\b[^>]*\bfreetv\.com/\?p=(\d+)"#,
        r#"(?is)<div\b[^>]*\bdata-params\s*=\s*["'][^"']*\bpost_id=(\d+)"#,
    ];
    patterns
        .iter()
        .find_map(|pattern| {
            Regex::new(pattern)
                .ok()
                .and_then(|matcher| matcher.captures(webpage).ok().flatten())
                .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FreeTV movie {display_id} has no video ID"),
            )
        })
}

fn freetv_hls_formats(stream_url: &str) -> Vec<serde_json::Value> {
    vec![serde_json::json!({
        "url": stream_url,
        "format_id": "hls",
        "protocol": "m3u8_native",
        "ext": "mp4",
    })]
}

fn freetv_episode_entry(
    episode: &serde_json::Value,
    series_title: Option<&str>,
) -> Option<InfoDict> {
    let video_id = json_value_string(episode.get("contentID"))?;
    let stream_url = json_string(episode, "streamURL")
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id.clone()));
    info.insert_if_some("title", json_string(episode, "fullTitle"));
    info.insert_if_some("description", json_string(episode, "description"));
    info.insert_if_some("thumbnail", json_string(episode, "thumbnail"));
    info.insert_if_some("series", series_title);
    let display_meta = episode
        .get("contentMeta")
        .and_then(|value| value.get("displayMeta"))
        .unwrap_or(&serde_json::Value::Null);
    info.insert_if_some(
        "series_id",
        json_value_string(display_meta.get("seriesID")),
    );
    info.insert_if_some(
        "season_id",
        json_value_string(display_meta.get("seasonID")),
    );
    info.insert_if_some("season_number", json_i64(display_meta, "seasonNum"));
    info.insert_if_some("episode_number", json_i64(display_meta, "episodeNum"));
    info.insert("url", serde_json::json!(stream_url));
    info.insert("ext", serde_json::json!("mp4"));
    info.insert(
        "formats",
        serde_json::Value::Array(freetv_hls_formats(stream_url)),
    );
    info.insert("subtitles", serde_json::json!({}));
    Some(info)
}

fn freetv_class_text(html: &str, tag: &str, class: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<{tag}\b[^>]*\bclass\s*=\s*["'][^"']*\b{}\b[^"']*["'][^>]*>(.*?)</{tag}\s*>"#,
        regex::escape(class)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| html_text_fragment(value.as_str())))
        .filter(|value| !value.is_empty())
}

fn freetv_match_id(
    matcher: &Regex,
    url: &str,
    label: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("{label} URL has no ID"),
            )
        })
}
