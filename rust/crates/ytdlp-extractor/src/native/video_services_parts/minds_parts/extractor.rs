fn minds_entity_video_id(
    entity_id: &str,
    entity: &serde_json::Value,
) -> Result<Result<String, String>, ExtractorError> {
    if json_string(entity, "type") == Some("activity") {
        if json_string(entity, "custom_type") == Some("video") {
            return Ok(Ok(minds_value_string(entity.get("entity_guid")).ok_or_else(
                || {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Minds activity {entity_id} has no video GUID"),
                    )
                },
            )?));
        }
        let perma_url = minds_valid_http_url(minds_value_string(entity.get("perma_url"))).ok_or_else(
            || {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Minds activity {entity_id} has no permalink"),
                )
            },
        )?;
        return Ok(Err(perma_url));
    }
    if json_string(entity, "subtype") == Some("video") {
        return Ok(Ok(entity_id.to_owned()));
    }
    Err(ExtractorError::new(
        ExtractorErrorKind::Extraction,
        format!("Minds entity {entity_id} is not a video"),
    ))
}

/// Native Minds video/entity API extractor.
pub struct MindsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MindsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MindsExtractor {
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
        let entity_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Minds URL has no entity ID")
            })?;
        let entity_payload = minds_api_json(
            context,
            &format!("v1/entities/entity/{entity_id}"),
            &[],
            "entity",
        )?;
        let entity = entity_payload.get("entity").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Minds entity {entity_id} response has no entity"),
            )
        })?;
        let video_id = match minds_entity_video_id(&entity_id, entity)? {
            Ok(video_id) => video_id,
            Err(perma_url) => {
                return Ok(ExtractorResult::Redirect {
                    url: perma_url,
                    ie_key: None,
                });
            }
        };
        let video = minds_api_json(
            context,
            &format!("v2/media/video/{video_id}"),
            &[],
            "video",
        )?;
        let formats = minds_formats(&video, &video_id)?;
        let video_entity = video
            .get("entity")
            .filter(|value| !value.is_null())
            .unwrap_or(entity);
        let owner = video_entity
            .get("ownerObj")
            .filter(|value| !value.is_null())
            .unwrap_or(&serde_json::Value::Null);
        let uploader_id = minds_value_string(owner.get("username"));
        let title = minds_text(video_entity.get("title")).unwrap_or_else(|| video_id.clone());
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let thumbnail_url = json_string(&video, "poster")
            .filter(|value| !value.is_empty())
            .or_else(|| json_string(video_entity, "thumbnail_src"))
            .and_then(|poster| context.get(poster).ok())
            .map(|response| response.url().to_owned());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("description", minds_text(video_entity.get("description")));
        info.insert_if_some("license", json_string(video_entity, "license"));
        info.insert_if_some("timestamp", minds_integer(video_entity.get("time_created")));
        info.insert_if_some("uploader", minds_text(owner.get("name")));
        info.insert_if_some("uploader_id", uploader_id.clone());
        info.insert_if_some("uploader_url", minds_uploader_url(uploader_id.as_deref()));
        info.insert_if_some(
            "view_count",
            minds_integer(video_entity.get("play:count")),
        );
        info.insert_if_some(
            "like_count",
            minds_integer(video_entity.get("thumbs:up:count")),
        );
        info.insert_if_some(
            "dislike_count",
            minds_integer(video_entity.get("thumbs:down:count")),
        );
        info.insert_if_some(
            "comment_count",
            minds_integer(video_entity.get("comments:count")),
        );
        info.insert_if_some("tags", minds_tags(video_entity.get("tags")));
        info.insert_if_some("thumbnail", thumbnail_url);
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        Ok(ExtractorResult::single(info))
    }
}

fn minds_feed_entries(
    context: &ExtractionContext,
    feed_guid: &str,
    feed_id: &str,
) -> Result<Vec<InfoDict>, ExtractorError> {
    const PAGE_SIZE: i64 = 150;
    let mut query = vec![
        ("limit".to_owned(), PAGE_SIZE.to_string()),
        ("sync".to_owned(), "1".to_owned()),
    ];
    let mut entries = Vec::new();
    let mut last_next = None;
    loop {
        let data = minds_api_json(
            context,
            &format!("v2/feeds/container/{feed_guid}/videos"),
            &query,
            &format!("feed {feed_id}"),
        )?;
        let entities = data
            .get("entities")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for entity in &entities {
            let Some(guid) = minds_value_string(entity.get("guid")) else {
                continue;
            };
            let mut entry = native_url_result(&format!("https://www.minds.com/newsfeed/{guid}"));
            entry.insert("ie_key", serde_json::json!("Minds"));
            entry.insert("id", serde_json::json!(guid));
            entries.push(entry);
        }
        if entities.len() != PAGE_SIZE as usize {
            break;
        }
        let Some(next) = minds_value_string(data.get("load-next")).filter(|value| !value.is_empty())
        else {
            break;
        };
        if last_next.as_deref() == Some(next.as_str()) {
            break;
        }
        last_next = Some(next.clone());
        query.retain(|(key, _)| key != "from_timestamp");
        query.push(("from_timestamp".to_owned(), next));
    }
    Ok(entries)
}

fn minds_feed_playlist(
    context: &ExtractionContext,
    feed_id: &str,
    feed_path: &str,
    feed_type: &str,
) -> Result<ExtractorResult, ExtractorError> {
    let feed_payload = minds_api_json(
        context,
        &format!("{feed_path}/{feed_id}"),
        &[],
        feed_type,
    )?;
    let feed = feed_payload.get(feed_type).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Minds {feed_type} {feed_id} response has no feed"),
        )
    })?;
    let feed_guid = minds_value_string(feed.get("guid")).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Minds {feed_type} {feed_id} has no feed GUID"),
        )
    })?;
    let entries = minds_feed_entries(context, &feed_guid, feed_id)?;
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(feed_id));
    info.insert_if_some("title", minds_text(feed.get("name")));
    info.insert_if_some(
        "description",
        minds_text(feed.get("briefdescription")),
    );
    Ok(ExtractorResult::Playlist { info, entries })
}

/// Native Minds channel video-feed playlist extractor.
pub struct MindsChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MindsChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MindsChannelExtractor {
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
        let feed_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Minds channel has no ID")
            })?;
        minds_feed_playlist(context, &feed_id, "v1/channel", "channel")
    }
}

/// Native Minds group video-feed playlist extractor.
pub struct MindsGroupExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MindsGroupExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MindsGroupExtractor {
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
        let feed_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Minds group has no ID")
            })?;
        minds_feed_playlist(context, &feed_id, "v1/groups/group", "group")
    }
}
