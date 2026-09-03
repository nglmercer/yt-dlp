/// Native YouTube playlist extraction for playlist URLs and bare IDs.
///
/// Mirrors the playlist branch of `YoutubeTabIE` in
/// `yt_dlp/extractor/youtube/_tab.py`: the initial page supplies `ytcfg` and
/// `ytInitialData`, the selected tab's `playlistVideoListRenderer` supplies
/// the first entries, and the `browse` API follows `nextContinuationData` /
/// `continuationItemRenderer` continuations until they run out or loop.
/// Channel, search, feed, hashtag, and music-tab URLs keep the `YoutubeTabIE`
/// TODO descriptor: only playlist-shaped pages are claimed.
///
/// Playlist entries are `url` results (`Youtube` video URLs with titles), so
/// the CLI resolves and downloads each video through the existing video
/// flow. Rich per-entry fields come from `youtube_extract_video` in
/// `entries.rs`; playlist header metadata beyond title/description/tags
/// stays TODO.
pub struct YoutubePlaylistExtractor {
    descriptor: ExtractorDescriptor,
}

impl YoutubePlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self { descriptor })
    }

    fn extract_playlist_native(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let playlist_id = youtube_playlist_id(url).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "YouTube playlist URL has no valid playlist ID",
            )
        })?;
        let page_url = format!("https://www.youtube.com/playlist?list={playlist_id}");
        let response = context.request(&youtube_page_request(&page_url))?;
        let webpage = String::from_utf8_lossy(response.body());
        let ytcfg = youtube_ytcfg(&webpage);
        let data = youtube_initial_data(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("YouTube playlist {playlist_id} has no initial data on its page"),
            )
        })?;
        let tabs = youtube_tab_renderers(&data);
        let selected = tabs
            .iter()
            .find(|tab| tab.get("selected").and_then(serde_json::Value::as_bool) == Some(true))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Unable to find selected tab",
                )
            })?;
        let visitor_data = youtube_configured_visitor_data(context, &[data.clone(), ytcfg.clone()]);
        let entries = youtube_collect_playlist_entries(
            context,
            &ytcfg,
            &playlist_id,
            selected,
            visitor_data,
        )?;
        Ok(ExtractorResult::Playlist {
            info: youtube_playlist_info(&data, &playlist_id),
            entries,
        })
    }
}

impl InfoExtractor for YoutubePlaylistExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        youtube_playlist_id(url).is_some()
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        usize::from(!self.descriptor.valid_urls.is_empty())
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        self.extract_playlist_native(url, context)
    }

    fn extract(&self, _url: &str) -> Result<InfoDict, ExtractorError> {
        Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            "TODO: native YouTube playlist extraction requires a request context",
        ))
    }
}

/// Extract the first `ytInitialData` object from a watch/playlist page,
/// mirroring `extract_yt_initial_data`.
fn youtube_initial_data(webpage: &str) -> Option<serde_json::Value> {
    json_objects_after_marker(webpage, "ytInitialData")
        .into_iter()
        .next()
}

/// List the tab renderers of a browse response, mirroring
/// `_extract_tab_renderers` (`tabRenderer` or `expandableTabRenderer`).
fn youtube_tab_renderers(data: &serde_json::Value) -> Vec<serde_json::Value> {
    data.get("contents")
        .and_then(|contents| contents.get("twoColumnBrowseResultsRenderer"))
        .and_then(|renderer| renderer.get("tabs"))
        .and_then(serde_json::Value::as_array)
        .map(|tabs| {
            tabs.iter()
                .filter_map(|tab| {
                    tab.get("tabRenderer")
                        .or_else(|| tab.get("expandableTabRenderer"))
                        .filter(|renderer| renderer.is_object())
                        .cloned()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Build one playlist entry URL result from a `playlistVideoRenderer` (or
/// `playlistPanelVideoRenderer`), keeping the video ID, canonical URL, and
/// runs-joined title. Entries without a video ID are skipped, mirroring
/// `_playlist_entries`.
fn youtube_playlist_entry(renderer: &serde_json::Value) -> Option<InfoDict> {
    let video_id = renderer
        .get("videoId")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())?;
    let mut entry = InfoDict::new();
    entry.insert("_type", serde_json::json!("url"));
    entry.insert("ie_key", serde_json::json!("Youtube"));
    entry.insert("id", serde_json::json!(video_id));
    entry.insert("url", serde_json::json!(youtube_canonical_url(video_id)));
    if let Some(title) = renderer.get("title").and_then(youtube_text) {
        entry.insert("title", serde_json::json!(title));
    }
    Some(entry)
}

/// Collect the entries of one `playlistVideoListRenderer`-shaped value:
/// every `playlistVideoRenderer`/`playlistPanelVideoRenderer` child with a
/// video ID becomes an entry, mirroring `_playlist_entries`.
fn youtube_playlist_list_entries(renderer: &serde_json::Value) -> Vec<InfoDict> {
    renderer
        .get("contents")
        .and_then(serde_json::Value::as_array)
        .map(|contents| {
            contents
                .iter()
                .filter_map(|content| {
                    content
                        .get("playlistVideoRenderer")
                        .or_else(|| content.get("playlistPanelVideoRenderer"))
                        .filter(|renderer| renderer.is_object())
                        .and_then(youtube_playlist_entry)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Build an API continuation query from a continuation endpoint's data,
/// mirroring `_extract_continuation_ep_data`: `{continuation, clickTracking?}`.
fn youtube_continuation_ep_data(endpoint: &serde_json::Value) -> Option<serde_json::Value> {
    let mut commands: Vec<&serde_json::Value> = endpoint
        .get("commandExecutorCommand")
        .and_then(|command| command.get("commands"))
        .and_then(serde_json::Value::as_array)
        .map(|commands| {
            commands
                .iter()
                .filter(|command| command.is_object())
                .collect()
        })
        .unwrap_or_default();
    if endpoint.is_object() {
        commands.push(endpoint);
    }
    commands.into_iter().find_map(|command| {
        let continuation = command
            .get("continuationCommand")?
            .get("token")?
            .as_str()
            .filter(|token| !token.is_empty())?;
        let mut query = serde_json::json!({ "continuation": continuation });
        if let Some(ctp) = command
            .get("clickTrackingParams")
            .and_then(serde_json::Value::as_str)
            .filter(|ctp| !ctp.is_empty())
        {
            query["clickTracking"] = serde_json::json!({ "clickTrackingParams": ctp });
        }
        Some(query)
    })
}

/// Extract the next continuation query from a renderer, mirroring
/// `_extract_continuation`: `continuations[0].nextContinuationData` or
/// `continuation.reloadContinuationData` first, then the
/// `continuationItemRenderer`/`continuationItemViewModel` scan over
/// `contents`/`items`/`rows`/`subThreads` in order.
pub(crate) fn youtube_extract_continuation(
    renderer: &serde_json::Value,
) -> Option<serde_json::Value> {
    let direct = renderer
        .get("continuations")
        .and_then(serde_json::Value::as_array)
        .and_then(|continuations| continuations.first())
        .and_then(|continuation| continuation.get("nextContinuationData"))
        .filter(|data| data.is_object())
        .or_else(|| {
            renderer
                .get("continuation")
                .and_then(|continuation| continuation.get("reloadContinuationData"))
                .filter(|data| data.is_object())
        });
    if let Some(data) = direct {
        let continuation = data
            .get("continuation")?
            .as_str()
            .filter(|token| !token.is_empty())?;
        let mut query = serde_json::json!({ "continuation": continuation });
        if let Some(ctp) = data
            .get("clickTrackingParams")
            .and_then(serde_json::Value::as_str)
            .filter(|ctp| !ctp.is_empty())
        {
            query["clickTracking"] = serde_json::json!({ "clickTrackingParams": ctp });
        }
        return Some(query);
    }
    for key in ["contents", "items", "rows", "subThreads"] {
        let items = renderer
            .get(key)
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for item in &items {
            if !item.is_object() {
                continue;
            }
            if let Some(renderer) = item.get("continuationItemRenderer") {
                let endpoint = renderer
                    .get("continuationEndpoint")
                    .or_else(|| {
                        renderer
                            .get("button")?
                            .get("buttonRenderer")?
                            .get("command")
                    })
                    .filter(|endpoint| endpoint.is_object());
                if let Some(query) = endpoint.and_then(youtube_continuation_ep_data) {
                    return Some(query);
                }
            }
            if let Some(query) = item
                .get("continuationItemViewModel")
                .and_then(|view_model| view_model.get("continuationCommand"))
                .and_then(|command| command.get("innertubeCommand"))
                .filter(|command| command.is_object())
                .and_then(youtube_continuation_ep_data)
            {
                return Some(query);
            }
        }
    }
    None
}

/// Read the first page of a selected playlist tab, mirroring the
/// `playlistVideoListRenderer` path of `_extract_entries`: entries plus the
/// renderer/is-renderer/parent continuation cascade.
fn youtube_first_playlist_page(
    tab: &serde_json::Value,
) -> (Vec<InfoDict>, Option<serde_json::Value>) {
    let section = tab
        .get("content")
        .and_then(|content| content.get("sectionListRenderer"));
    let contents = section
        .and_then(|section| section.get("contents"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for content in &contents {
        let section_renderer = content.get("itemSectionRenderer").filter(|renderer| {
            renderer
                .get("contents")
                .and_then(serde_json::Value::as_array)
                .is_some()
        });
        let Some(section_renderer) = section_renderer else {
            continue;
        };
        let items = section_renderer
            .get("contents")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for item in &items {
            let Some(list_renderer) = item
                .get("playlistVideoListRenderer")
                .filter(|renderer| renderer.is_object())
            else {
                continue;
            };
            let entries = youtube_playlist_list_entries(list_renderer);
            // Mirrors the continuation cascade: renderer, then the section
            // renderer, then the whole section list.
            let continuation = youtube_extract_continuation(list_renderer)
                .or_else(|| youtube_extract_continuation(section_renderer))
                .or_else(|| {
                    youtube_extract_continuation(section.unwrap_or(&serde_json::Value::Null))
                });
            return (entries, continuation);
        }
    }
    (Vec::new(), None)
}

/// Read one browse continuation page, mirroring the `_entries` response half:
/// entries come from leading `playlistVideoRenderer` items, and the next
/// continuation is scanned from the item list.
fn youtube_continuation_playlist_page(
    response: &serde_json::Value,
) -> (Vec<InfoDict>, Option<serde_json::Value>) {
    // Mirrors the `traverse_obj(..., get_all=False)` fallback chain:
    // `onResponseReceivedActions`, then `onResponseReceivedEndpoints`, then
    // `continuationContents`. Empty results fall through to the next path.
    let mut items = Vec::new();
    for key in ["onResponseReceivedActions", "onResponseReceivedEndpoints"] {
        items = response
            .get(key)
            .and_then(serde_json::Value::as_array)
            .map(|actions| {
                actions
                    .iter()
                    .filter_map(|action| {
                        action
                            .get("appendContinuationItemsAction")?
                            .get("continuationItems")
                            .and_then(serde_json::Value::as_array)
                            .cloned()
                    })
                    .flatten()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !items.is_empty() {
            break;
        }
    }
    if items.is_empty() {
        items = response
            .get("continuationContents")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
    }
    let first = items.first().cloned().unwrap_or(serde_json::Value::Null);
    let first_is_video = first
        .get("playlistVideoRenderer")
        .or_else(|| first.get("playlistPanelVideoRenderer"))
        .is_some();
    if first_is_video {
        let entries = items
            .iter()
            .filter_map(|item| {
                item.get("playlistVideoRenderer")
                    .or_else(|| item.get("playlistPanelVideoRenderer"))
                    .and_then(youtube_playlist_entry)
            })
            .collect();
        let continuation = youtube_extract_continuation(&serde_json::json!({ "contents": items }));
        return (entries, continuation);
    }
    // Mirrors the continuation-only branch (yt-dlp#12933): follow the token
    // without yielding entries.
    let continuation = youtube_extract_continuation(&serde_json::json!({ "contents": [first] }));
    (Vec::new(), continuation)
}

/// POST one browse continuation query, mirroring `_call_api` with
/// `ep='browse'` (context merged under the query, visitor header included).
fn youtube_browse_api_response(
    context: &ExtractionContext,
    ytcfg: &serde_json::Value,
    query: &serde_json::Value,
    visitor_data: Option<&str>,
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = youtube_api_endpoint(ytcfg, "browse", "playlist page")?;
    let api_context = youtube_api_context(ytcfg);
    let client_version = api_context
        .get("client")
        .and_then(|client| client.get("clientVersion"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(YOUTUBE_DEFAULT_CLIENT_VERSION)
        .to_owned();
    let mut payload = serde_json::json!({ "context": api_context });
    if let (Some(payload_object), Some(query_object)) = (payload.as_object_mut(), query.as_object())
    {
        for (key, value) in query_object {
            payload_object.insert(key.clone(), value.clone());
        }
    }
    let mut request = Request::new(endpoint);
    request.set_method("POST").map_err(map_request_error)?;
    request.headers_mut().set("Accept", "application/json");
    request
        .headers_mut()
        .set("Content-Type", "application/json");
    request
        .headers_mut()
        .set("Origin", "https://www.youtube.com");
    request.headers_mut().set("X-YouTube-Client-Name", "1");
    request
        .headers_mut()
        .set("X-YouTube-Client-Version", client_version.as_str());
    if let Some(visitor_data) = visitor_data.filter(|value| !value.is_empty()) {
        request.headers_mut().set("X-Goog-Visitor-Id", visitor_data);
    }
    request
        .headers_mut()
        .set("User-Agent", YOUTUBE_DEFAULT_USER_AGENT);
    request.set_data(Some(serde_json::to_vec(&payload).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("could not encode YouTube browse API request: {error}"),
        )
    })?));
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid YouTube browse API response: {error}"),
        )
    })
}

/// Collect every entry of the selected playlist tab, mirroring `_entries`:
/// the first page comes from the tab renderers, then continuation queries
/// are POSTed until none remain or a token repeats (feed-loop guard).
/// Visitor data refreshes from each response, mirroring the infinite-loop
/// guard from youtube-dl#28702.
fn youtube_collect_playlist_entries(
    context: &ExtractionContext,
    ytcfg: &serde_json::Value,
    playlist_id: &str,
    tab: &serde_json::Value,
    visitor_data: Option<String>,
) -> Result<Vec<InfoDict>, ExtractorError> {
    let (mut entries, mut continuation) = youtube_first_playlist_page(tab);
    let mut visitor_data = visitor_data;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut page_num = 0u32;
    while let Some(query) = continuation.take() {
        let token = query
            .get("continuation")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if token.is_empty() || !seen.insert(token) {
            break;
        }
        page_num += 1;
        let response = youtube_browse_api_response(context, ytcfg, &query, visitor_data.as_deref())
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("YouTube playlist {playlist_id} page {page_num}: {error}"),
                )
            })?;
        visitor_data = youtube_visitor_data(std::slice::from_ref(&response)).or(visitor_data);
        let (mut page_entries, next) = youtube_continuation_playlist_page(&response);
        entries.append(&mut page_entries);
        continuation = next;
        if continuation.is_none() {
            // Mirrors `if not continuation and not video_items_renderer`:
            // pages without entries or follow-ups end the feed. An entries
            // page whose continuation scan came up empty ends here too.
            break;
        }
    }
    Ok(entries)
}

/// Playlist identity metadata, mirroring the `playlistMetadataRenderer` core
/// of `_extract_metadata_from_tabs`. Channel/uploader/count/badge fields need
/// sidebar and header renderers plus the badge taxonomy and stay TODOs.
fn youtube_playlist_info(data: &serde_json::Value, playlist_id: &str) -> InfoDict {
    let metadata = data
        .get("metadata")
        .and_then(|metadata| metadata.get("playlistMetadataRenderer"));
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(playlist_id));
    let title = metadata
        .and_then(|metadata| metadata.get("title"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            data.get("header")
                .and_then(|header| header.get("hashtagHeaderRenderer"))
                .and_then(|header| header.get("hashtag"))
                .and_then(youtube_text)
        })
        .unwrap_or_else(|| playlist_id.to_owned());
    info.insert("title", serde_json::json!(title));
    info.insert(
        "description",
        metadata
            .and_then(|metadata| metadata.get("description"))
            .and_then(serde_json::Value::as_str)
            .map_or(serde_json::json!(""), |description| {
                serde_json::json!(description)
            }),
    );
    let tags = data
        .get("microformat")
        .and_then(|microformat| microformat.get("microformatDataRenderer"))
        .and_then(|renderer| renderer.get("tags"))
        .and_then(serde_json::Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    info.insert("tags", serde_json::json!(tags));
    info
}
