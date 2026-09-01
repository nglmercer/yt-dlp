/// Native JioSaavn album playlist extractor.
pub struct JioSaavnAlbumExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JioSaavnAlbumExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JioSaavnAlbumExtractor {
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
        let album_id = jiosaavn_match_id(&self.matcher, url, "album")?;
        let album = jiosaavn_call_api(context, "album", &album_id, &[])?;
        let entries = jiosaavn_items(&album, Some("songs"))
            .into_iter()
            .filter_map(|item| jiosaavn_item_entry(item, false))
            .collect();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(album_id));
        info.insert_if_some("title", json_string(&album, "title"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native JioSaavn paged playlist extractor.
pub struct JioSaavnPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JioSaavnPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JioSaavnPlaylistExtractor {
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
        let playlist_id = jiosaavn_match_id(&self.matcher, url, "playlist")?;
        let page_size = 50_i64;
        let first_page = jiosaavn_call_api(
            context,
            "playlist",
            &playlist_id,
            &[("p", "1".to_owned()), ("n", page_size.to_string())],
        )?;
        let list_count = jiosaavn_integer(first_page.get("list_count")).unwrap_or_default();
        let total_pages = ((list_count + page_size - 1) / page_size).max(1);
        let mut entries = jiosaavn_page_entries(&first_page, Some("songs"), false);
        for page in 2..=total_pages {
            let page_data = jiosaavn_call_api(
                context,
                "playlist",
                &playlist_id,
                &[("p", page.to_string()), ("n", page_size.to_string())],
            )?;
            entries.extend(jiosaavn_page_entries(&page_data, Some("songs"), false));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", json_string(&first_page, "listname"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native JioSaavn show-season playlist extractor.
pub struct JioSaavnShowPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JioSaavnShowPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JioSaavnShowPlaylistExtractor {
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
                "JioSaavn show playlist URL did not match",
            )
        })?;
        let show_slug = captures
            .name("show")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "JioSaavn show playlist has no show slug",
                )
            })?;
        let season_id = captures
            .name("season")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "JioSaavn show playlist has no season number",
                )
            })?;
        let page_size = 10_i64;
        let show_info = jiosaavn_show_info(context, url)?;
        let show_id = json_value_string(
            show_info
                .get("current_id")
                .or_else(|| show_info.get("id")),
        )
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JioSaavn show {show_slug} has no show ID"),
            )
        })?;
        let mut entries = Vec::new();
        for page in 1.. {
            let page_data = jiosaavn_show_page(context, &show_id, &season_id, page, page_size)?;
            let page_entries = jiosaavn_page_entries(&page_data, None, true);
            let empty = page_entries.is_empty();
            entries.extend(page_entries);
            if empty {
                break;
            }
        }
        let playlist_id = format!("{show_slug}-{season_id}");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some(
            "title",
            show_info
                .get("show")
                .and_then(|show| show.get("title"))
                .and_then(|title| title.get("text"))
                .and_then(serde_json::Value::as_str),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native JioSaavn artist top-songs playlist extractor.
pub struct JioSaavnArtistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JioSaavnArtistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JioSaavnArtistExtractor {
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
        let artist_id = jiosaavn_match_id(&self.matcher, url, "artist")?;
        let page_size = 50_i64;
        let mut entries = Vec::new();
        let mut page = 0_i64;
        let mut first_page = None;
        loop {
            let page_data = jiosaavn_artist_page(context, &artist_id, page, page_size)?;
            if first_page.is_none() {
                first_page = Some(page_data.clone());
            }
            let page_entries = jiosaavn_page_entries(&page_data, Some("topSongs"), false);
            if page_entries.is_empty() {
                break;
            }
            entries.extend(page_entries);
            page += 1;
        }
        let first_page = first_page.unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(artist_id));
        info.insert_if_some("title", json_string(&first_page, "name"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn jiosaavn_match_id(
    matcher: &Regex,
    url: &str,
    kind: &str,
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
                format!("JioSaavn {kind} URL has no ID"),
            )
        })
}

fn jiosaavn_page_entries(
    page: &serde_json::Value,
    key: Option<&str>,
    episode: bool,
) -> Vec<InfoDict> {
    jiosaavn_items(page, key)
        .into_iter()
        .filter_map(|item| jiosaavn_item_entry(item, episode))
        .collect()
}

fn jiosaavn_show_info(
    context: &ExtractionContext,
    url: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let response = context.get(url)?;
    let webpage = String::from_utf8_lossy(response.body());
    json_object_after_marker(&webpage, "window.__INITIAL_DATA__").and_then(|data| data.get("showView").cloned()).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "JioSaavn show page has no initial data",
        )
    })
}

fn jiosaavn_show_page(
    context: &ExtractionContext,
    show_id: &str,
    season_id: &str,
    page: i64,
    page_size: i64,
) -> Result<serde_json::Value, ExtractorError> {
    jiosaavn_call_api(
        context,
        "show",
        show_id,
        &[
            ("p", page.to_string()),
            ("__call", "show.getAllEpisodes".to_owned()),
            ("show_id", show_id.to_owned()),
            ("season_number", season_id.to_owned()),
            ("api_version", "4".to_owned()),
            ("sort_order", "desc".to_owned()),
            ("n", page_size.to_string()),
        ],
    )
}

fn jiosaavn_artist_page(
    context: &ExtractionContext,
    artist_id: &str,
    page: i64,
    page_size: i64,
) -> Result<serde_json::Value, ExtractorError> {
    jiosaavn_call_api(
        context,
        "artist",
        artist_id,
        &[
            ("p", page.to_string()),
            ("n_song", page_size.to_string()),
            ("n_album", page_size.to_string()),
            ("sub_type", String::new()),
            ("includeMetaTags", String::new()),
            ("api_version", "4".to_owned()),
            ("category", "alphabetical".to_owned()),
            ("sort_order", "asc".to_owned()),
        ],
    )
}
