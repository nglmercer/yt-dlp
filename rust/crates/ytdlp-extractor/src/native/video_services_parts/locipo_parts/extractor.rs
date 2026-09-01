pub struct LocipoExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

pub struct LocipoPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

impl LocipoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let matchers = descriptor
            .valid_urls
            .iter()
            .map(|pattern| {
                compile_source_pattern(pattern).map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("invalid Locipo URL pattern: {error}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            descriptor,
            matchers,
        })
    }
}

impl LocipoPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let matchers = descriptor
            .valid_urls
            .iter()
            .map(|pattern| {
                compile_source_pattern(pattern).map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("invalid Locipo playlist URL pattern: {error}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            descriptor,
            matchers,
        })
    }
}

fn locipo_capture<'a>(
    matchers: &[Regex],
    url: &'a str,
) -> Option<fancy_regex::Captures<'a>> {
    matchers
        .iter()
        .find_map(|matcher| matcher.captures(url).ok().flatten())
}

fn locipo_video_id(matchers: &[Regex], url: &str) -> Result<String, ExtractorError> {
    locipo_capture(matchers, url)
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Locipo URL has no creative ID")
        })
}

fn locipo_keyword_values(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let value = value?.as_str()?;
    let values = value
        .split(',')
        .map(|value| html_text_fragment(value.trim()))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn locipo_creative_info(
    mut info: InfoDict,
    creative: &serde_json::Value,
    video_id: &str,
) -> InfoDict {
    info.insert("id", serde_json::json!(video_id));
    info.insert_if_some("title", json_string(creative, "name").map(html_text_fragment));
    info.insert_if_some(
        "description",
        json_string(creative, "description").map(html_text_fragment),
    );
    info.insert_if_some(
        "release_timestamp",
        json_string(creative, "publication_started_at")
            .and_then(|value| parse_timestamp(value.to_owned())),
    );
    info.insert_if_some("tags", locipo_keyword_values(creative.get("keyword")));
    if let Some(company) = creative.get("company") {
        info.insert_if_some("uploader", json_string(company, "name").map(html_text_fragment));
    }
    if let Some(series) = creative.get("series") {
        info.insert_if_some("series", json_string(series, "name").map(html_text_fragment));
        info.insert_if_some("series_id", json_value_string(series.get("id")));
    }
    info
}

impl InfoExtractor for LocipoExtractor {
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
        let video_id = locipo_video_id(&self.matchers, url)?;
        if let Some(playlist_id) = url_query_value(url, "list") {
            if !playlist_id.is_empty() {
                return Ok(ExtractorResult::Redirect {
                    url: format!("{LOCIPO_BASE_URL}/playlist/{playlist_id}"),
                    ie_key: Some("LocipoPlaylist".to_owned()),
                });
            }
        }
        let creative = locipo_creative(context, &video_id)?;
        let media_id = json_value_string(creative.get("media_id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Locipo creative {video_id} has no Streaks media ID"),
            )
        })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let api_key = locipo_api_key(&webpage, &video_id)?;
        let playback = locipo_streaks_playback(context, &media_id, &api_key)?;
        let info = locipo_streaks_info(&playback, &media_id)?;
        Ok(ExtractorResult::single(locipo_creative_info(
            info, &creative, &video_id,
        )))
    }
}

fn locipo_playlist_type_and_id(
    matchers: &[Regex],
    url: &str,
) -> Result<(String, String), ExtractorError> {
    let captures = locipo_capture(matchers, url).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            "Locipo playlist URL did not match",
        )
    })?;
    let playlist_type = captures
        .name("type")
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Locipo playlist URL has no type",
            )
        })?;
    let playlist_id = captures
        .name("id")
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Locipo playlist URL has no ID",
            )
        })?;
    Ok((playlist_type, playlist_id))
}

fn locipo_playlist_entries(
    response: &serde_json::Value,
    playlist_type: &str,
    playlist_id: &str,
) -> Result<Vec<InfoDict>, ExtractorError> {
    let items = response
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Locipo {playlist_type} {playlist_id} has no creative items"),
            )
        })?;
    let mut entries = Vec::new();
    for item in items {
        let Some(video_id) = json_value_string(item.get("id")) else {
            continue;
        };
        let mut entry = native_url_result(&format!("{LOCIPO_BASE_URL}/creative/{video_id}"));
        entry.insert("ie_key", serde_json::json!("Locipo"));
        entries.push(entry);
    }
    Ok(entries)
}

fn locipo_playlist_metadata(
    response: &serde_json::Value,
    playlist_type: &str,
) -> (Option<String>, Option<String>) {
    response
        .get("items")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|item| {
            let playlist = item.get(playlist_type)?;
            Some((
                json_string(playlist, "name").map(html_text_fragment),
                json_string(playlist, "description").map(html_text_fragment),
            ))
        })
        .unwrap_or((None, None))
}

impl InfoExtractor for LocipoPlaylistExtractor {
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
        let (playlist_type, playlist_id) = locipo_playlist_type_and_id(&self.matchers, url)?;
        let initial = locipo_playlist_page(context, &playlist_type, &playlist_id, 1)?;
        let total = json_i64(&initial, "total").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Locipo {playlist_type} {playlist_id} has no total count"),
            )
        })?;
        let pages = if total <= 0 {
            1
        } else {
            (total + LOCIPO_PAGE_SIZE - 1) / LOCIPO_PAGE_SIZE
        };
        let mut entries = locipo_playlist_entries(&initial, &playlist_type, &playlist_id)?;
        for page in 2..=pages {
            let response = locipo_playlist_page(context, &playlist_type, &playlist_id, page)?;
            entries.extend(locipo_playlist_entries(
                &response,
                &playlist_type,
                &playlist_id,
            )?);
        }
        let (title, description) = locipo_playlist_metadata(&initial, &playlist_type);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
