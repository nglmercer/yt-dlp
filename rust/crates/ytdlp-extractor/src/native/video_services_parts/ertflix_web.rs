/// Native ERTFLIX web-page and series extractor.
pub struct ErtflixExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ErtflixExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ErtflixExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "ERTFLIX URL has no content ID")
            })?;
        if video_id.starts_with("ser.") {
            let (season_titles, season_numbers) = ertflix_season_filters(url);
            let media_info = ertflix_api_request(
                context,
                "Tile/GetSeriesDetails",
                1,
                &[("id".to_owned(), video_id.clone())],
            )?;
            let series = media_info.get("Series").unwrap_or(&serde_json::Value::Null);
            let mut allowed_titles = season_titles;
            if !season_numbers.is_empty() {
                if let Some(seasons) = series.get("Seasons").and_then(serde_json::Value::as_array) {
                    for season in seasons {
                        let Some(number) = json_i64(season, "SeasonNumber") else {
                            continue;
                        };
                        if !season_numbers.contains(&number) {
                            continue;
                        }
                        if let Some(title) =
                            json_string(season, "Title").filter(|value| !value.is_empty())
                        {
                            if !allowed_titles.contains(&title.to_owned()) {
                                allowed_titles.push(title.to_owned());
                            }
                        }
                    }
                }
            }
            let mut entries = Vec::new();
            if let Some(groups) = media_info
                .get("EpisodeGroups")
                .and_then(serde_json::Value::as_array)
            {
                for group in groups {
                    let group_title = json_string(group, "Title").unwrap_or("");
                    if !allowed_titles.is_empty()
                        && !allowed_titles.iter().any(|title| title == group_title)
                    {
                        continue;
                    }
                    let Some(episodes) =
                        group.get("Episodes").and_then(serde_json::Value::as_array)
                    else {
                        continue;
                    };
                    if episodes.is_empty() {
                        continue;
                    }
                    let all_numbered = episodes
                        .iter()
                        .all(|episode| json_i64(episode, "EpisodeNumber").is_some());
                    let mut ordered = episodes.iter().collect::<Vec<_>>();
                    if all_numbered {
                        ordered.sort_by_key(|episode| {
                            json_i64(episode, "EpisodeNumber").unwrap_or_default()
                        });
                    }
                    for (index, episode) in ordered.into_iter().enumerate() {
                        let episode_number = if all_numbered {
                            json_i64(episode, "EpisodeNumber")
                        } else {
                            Some((index + 1) as i64)
                        };
                        if let Some(entry) =
                            ertflix_episode_entry(episode, Some(group), episode_number)
                        {
                            entries.push(entry);
                        }
                    }
                }
            }
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(video_id));
            info.insert_if_some("title", json_string(series, "Title"));
            info.insert_if_some(
                "description",
                ertflix_first_string(series, &["ShortDescription", "TinyDescription"]),
            );
            info.insert_if_some("age_limit", ertflix_age_limit(series));
            return Ok(ExtractorResult::Playlist { info, entries });
        }

        let media_info = ertflix_api_post_request(
            context,
            "Tile/GetTiles",
            2,
            &serde_json::json!({"RequestedTiles":[{"Id":video_id}]}),
        )?;
        let tile = media_info
            .get("Tiles")
            .and_then(serde_json::Value::as_array)
            .and_then(|tiles| {
                tiles.iter().find(|tile| {
                    json_value_string(tile.get("Id")).as_deref() == Some(video_id.as_str())
                })
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ERTFLIX content {video_id} has no matching tile"),
                )
            })?;
        let entry = ertflix_episode_entry(tile, None, None).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ERTFLIX content {video_id} has no playable episode"),
            )
        })?;
        Ok(ExtractorResult::single(entry))
    }
}

fn ertflix_api_post_request(
    context: &ExtractionContext,
    method: &str,
    api_version: u8,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("https://api.app.ertflix.gr/v{api_version}/{method}");
    let mut request = Request::new(&endpoint);
    let headers = serde_json::json!({
        "X-Api-Date-Format": "iso",
        "X-Api-Camel-Case": false,
        "Content-Type": "application/json;charset=utf-8",
    })
    .to_string();
    request.update_query(&[("$headers".to_owned(), headers)]);
    let mut body = serde_json::Map::new();
    body.insert("platformCodename".to_owned(), serde_json::json!("www"));
    if let Some(payload) = payload.as_object() {
        body.extend(payload.clone());
    }
    request.set_method("POST").map_err(map_request_error)?;
    request
        .headers_mut()
        .set("Content-Type", "application/json;charset=utf-8");
    request.set_data(Some(serde_json::to_vec(&serde_json::Value::Object(body)).map_err(
        |error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("could not encode ERTFLIX API request: {error}"),
            )
        },
    )?));
    let response = context.request(&request)?;
    ertflix_decode_api_response(response.body(), response.url())
}

fn ertflix_decode_api_response(
    body: &[u8],
    response_url: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let data = serde_json::from_slice::<serde_json::Value>(body).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid ERTFLIX API JSON from {response_url}: {error}"),
        )
    })?;
    if data
        .get("Result")
        .and_then(|result| json_bool(result, "Success"))
        != Some(true)
    {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "ERTFLIX API rejected the content request",
        ));
    }
    Ok(data)
}

fn ertflix_episode_entry(
    episode: &serde_json::Value,
    group: Option<&serde_json::Value>,
    episode_number: Option<i64>,
) -> Option<InfoDict> {
    let codename = json_string(episode, "Codename")
        .filter(|value| !value.is_empty())?
        .to_owned();
    let title = json_string(episode, "Title")
        .filter(|value| !value.is_empty())?
        .to_owned();
    if json_bool(episode, "HasPlayableStream") == Some(false) {
        return None;
    }
    let mut entry = native_url_result(&format!("ertflix:{codename}"));
    entry.insert("_type", serde_json::json!("url_transparent"));
    entry.insert("id", serde_json::json!(codename));
    entry.insert_if_some("episode_id", json_value_string(episode.get("Id")));
    entry.insert("title", serde_json::json!(title));
    entry.insert_if_some("alt_title", json_string(episode, "Subtitle"));
    entry.insert_if_some(
        "description",
        ertflix_first_string(episode, &["ShortDescription", "TinyDescription"])
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty()),
    );
    entry.insert_if_some(
        "timestamp",
        json_string(episode, "PublishDate")
            .map(str::to_owned)
            .and_then(parse_timestamp),
    );
    entry.insert_if_some("duration", json_f64(episode, "DurationSeconds"));
    entry.insert_if_some("age_limit", ertflix_age_limit(episode));
    entry.insert_if_some("thumbnail", ertflix_main_thumbnail(episode));
    if let Some(episode_number) = episode_number {
        entry.insert("episode_number", serde_json::json!(episode_number));
    }
    if let Some(group) = group {
        entry.insert_if_some("season", json_string(group, "Title"));
        entry.insert_if_some("season_number", json_i64(group, "SeasonNumber"));
    }
    Some(entry)
}

fn ertflix_first_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| json_string(value, key).filter(|value| !value.is_empty()))
        .map(str::to_owned)
}

fn ertflix_age_limit(value: &serde_json::Value) -> Option<i64> {
    json_i64(value, "AgeRating")
        .or_else(|| json_bool(value, "IsAdultContent").filter(|value| *value).map(|_| 18))
        .or_else(|| json_bool(value, "IsKidsContent").filter(|value| *value).map(|_| 0))
}

fn ertflix_main_thumbnail(value: &serde_json::Value) -> Option<String> {
    let root = value.get("Images").or_else(|| value.get("Image"))?;
    let mut images = Vec::new();
    ertflix_image_values(root, &mut images);
    images.into_iter().find_map(|image| {
        if json_bool(image, "IsMain") != Some(true) {
            return None;
        }
        json_string(image, "Url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
    })
}

fn ertflix_image_values<'a>(
    value: &'a serde_json::Value,
    output: &mut Vec<&'a serde_json::Value>,
) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                ertflix_image_values(value, output);
            }
        }
        serde_json::Value::Object(values) => {
            if let Some(images) = values.get("Image") {
                ertflix_image_values(images, output);
            } else {
                output.push(value);
            }
        }
        _ => {}
    }
}

fn ertflix_season_filters(url: &str) -> (Vec<String>, Vec<i64>) {
    let mut titles = Vec::new();
    let mut numbers = Vec::new();
    if let Ok(parsed) = url::Url::parse(url) {
        for (key, value) in parsed.query_pairs() {
            if key != "season" {
                continue;
            }
            if let Ok(number) = value.parse::<i64>() {
                numbers.push(number);
            } else if !value.is_empty() {
                titles.push(value.into_owned());
            }
        }
    }
    (titles, numbers)
}
