/// Native standard YouTube video extractor.
///
/// This native boundary covers video URLs only. Playlist URLs and bare IDs
/// are claimed by `YoutubePlaylistExtractor` (see `playlist.rs`); search,
/// feed, clip, channel-tab, and account URLs retain their generated
/// descriptors and remain explicit TODOs.
pub struct YoutubeExtractor {
    descriptor: ExtractorDescriptor,
}

impl YoutubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self { descriptor })
    }

    fn extract_native(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = youtube_video_id(url).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "YouTube video URL has no valid 11-character video ID",
            )
        })?;
        let webpage_url = youtube_canonical_url(&video_id);
        let response = context.request(&youtube_page_request(&webpage_url))?;
        let webpage = String::from_utf8_lossy(response.body());
        let ytcfg = youtube_ytcfg(&webpage);
        let initial_responses = youtube_player_responses_from_page(&webpage, &video_id);
        let initial_response = youtube_select_player_response(&initial_responses, &video_id);
        let player_response = if initial_response
            .as_ref()
            .is_some_and(youtube_response_has_streaming_data)
        {
            initial_response.clone().expect("checked above")
        } else {
            // Configured session inputs mirror `_real_extract`: the
            // `visitor_data` extractor-arg overrides page extraction, and a
            // configured Player PO Token rides the player request.
            // TODO: fetch Player/GVS tokens from the PO-token director when
            // no configured token exists (`fetch_pot` policy pending).
            let mut visitor_candidates = vec![ytcfg.clone()];
            visitor_candidates.extend(initial_responses.iter().cloned());
            let visitor_data = youtube_configured_visitor_data(context, &visitor_candidates);
            let player_po_token = youtube_configured_player_po_token(context);
            youtube_api_response(
                context,
                &ytcfg,
                &video_id,
                player_po_token.as_deref(),
                visitor_data.as_deref(),
            )
            .or_else(|error| {
                if initial_response.is_some() {
                    Ok(initial_response.clone().expect("checked above"))
                } else {
                    Err(error)
                }
            })?
        };

        let mut responses = initial_responses;
        if !responses.iter().any(|response| response == &player_response) {
            responses.push(player_response.clone());
        }
        // Rental trailers resolve to the trailer video itself, mirroring the
        // `playerLegacyDesktopYpcTrailerRenderer` branch in `_real_extract`.
        if let Some(trailer) = responses.iter().find_map(|response| {
            response
                .get("playabilityStatus")?
                .get("errorScreen")?
                .get("playerLegacyDesktopYpcTrailerRenderer")?
                .get("trailerVideoId")?
                .as_str()
        }) {
            return Ok(ExtractorResult::Redirect {
                url: youtube_canonical_url(trailer),
                ie_key: Some("Youtube".to_owned()),
            });
        }

        let details = responses
            .iter()
            .find_map(|response| response.get("videoDetails"))
            .unwrap_or(&serde_json::Value::Null);
        let microformat = responses
            .iter()
            .find_map(|response| {
                response
                    .get("microformat")
                    .and_then(|microformat| microformat.get("playerMicroformatRenderer"))
            })
            .unwrap_or(&serde_json::Value::Null);
        let title = youtube_json_string(details, "title")
            .or_else(|| youtube_json_string(microformat, "title"))
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .map(|title| html_text_fragment(&title))
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| video_id.clone());
        let description = youtube_json_string(details, "shortDescription")
            .or_else(|| youtube_json_string(microformat, "description"))
            .or_else(|| html_meta_value(&webpage, "og:description"))
            .map(|description| html_text_fragment(&description));
        let channel_id = youtube_json_string(details, "channelId")
            .or_else(|| youtube_json_string(microformat, "externalChannelId"));
        let is_live = details.get("isLive").and_then(serde_json::Value::as_bool);
        let is_live_content = details
            .get("isLiveContent")
            .and_then(serde_json::Value::as_bool);
        let is_upcoming = details
            .get("isUpcoming")
            .and_then(serde_json::Value::as_bool);
        let is_post_live = details
            .get("isPostLiveDvr")
            .and_then(serde_json::Value::as_bool);
        // Mirrors `_list_formats`: the status stays unknown when neither flag
        // is present at all.
        let live_status = if is_post_live == Some(true) {
            Some("post_live")
        } else if is_live == Some(true) {
            Some("is_live")
        } else if is_upcoming == Some(true) {
            Some("is_upcoming")
        } else if is_live_content == Some(true) {
            Some("was_live")
        } else if [is_live, is_live_content].contains(&Some(false)) {
            Some("not_live")
        } else {
            None
        };
        let live_details = microformat.get("liveBroadcastDetails");
        let mut duration = youtube_json_i64(details, "lengthSeconds")
            .or_else(|| youtube_json_i64(microformat, "lengthSeconds"));
        if duration.is_none() {
            // Live streams without `lengthSeconds` span their broadcast window.
            if let Some(live_details) = live_details {
                let start =
                    youtube_json_string(live_details, "startTimestamp").and_then(parse_timestamp);
                let end =
                    youtube_json_string(live_details, "endTimestamp").and_then(parse_timestamp);
                duration = match (start, end) {
                    (Some(start), Some(end)) if end > start => Some(end - start),
                    _ => None,
                };
            }
        }
        let (mut formats, mut todos, challenges) =
            youtube_formats_and_todos(&responses, duration, live_status);
        if formats.is_empty() {
            return Err(youtube_no_formats_error(&video_id, &responses, &todos));
        }
        // Bulk-solve signature/`n` challenges like `solve_js_challenges`,
        // then prune the TODO groups that were fully solved. Anything
        // unsolved (or unsolvable without a runtime) keeps its TODO, with
        // per-request failures surfaced explicitly.
        let mut player = None;
        if !challenges.is_empty() {
            match youtube_resolve_player(context, &ytcfg) {
                Ok(resolved) => player = Some(resolved),
                Err(error) => {
                    let message = error.to_string();
                    todos.push(if message.starts_with("TODO:") {
                        message
                    } else {
                        format!("TODO: YouTube player inventory failed: {message}")
                    });
                }
            }
        }
        if let Some(player) = player.as_ref() {
            if let Some(script) = player.script.as_deref() {
                match youtube_bulk_solve(script, &challenges) {
                    Ok(solutions) => {
                        let (sig_done, n_done) =
                            youtube_apply_solutions(&mut formats, &challenges, &solutions);
                        if sig_done {
                            todos.retain(|todo| !todo.contains("signatureCipher"));
                        }
                        if n_done {
                            todos.retain(|todo| !todo.contains("n challenge"));
                        }
                        for error in &solutions.errors {
                            todos.push(format!("TODO: YouTube challenge solver failed: {error}"));
                        }
                    }
                    Err(error) => todos.push(error.to_string()),
                }
            }
        }
        let (subtitles, automatic_captions) = youtube_caption_entries(&player_response);
        let mut thumbnails = details
            .get("thumbnail")
            .and_then(|thumbnail| thumbnail.get("thumbnails"))
            .and_then(serde_json::Value::as_array)
            .map(|thumbnails| {
                thumbnails
                    .iter()
                    .filter_map(|thumbnail| {
                        let url = thumbnail.get("url").and_then(serde_json::Value::as_str)?;
                        Some(serde_json::json!({
                            "url": url,
                            "width": thumbnail.get("width"),
                            "height": thumbnail.get("height"),
                        }))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        // The thumbnail is the last sure-to-exist original, captured before
        // the synthesized candidates are appended.
        let thumbnail = thumbnails
            .iter()
            .filter_map(|thumbnail| thumbnail.get("url").cloned())
            .last();
        // The best-resolution thumbnails sometimes do not appear in the page
        // data, so deterministic `i.ytimg.com` candidates are appended and
        // ranked exactly like `_real_extract`.
        const THUMBNAIL_NAMES: &[&str] = &[
            "maxresdefault",
            "hq720",
            "sddefault",
            "hqdefault",
            "0",
            "mqdefault",
            "default",
            "sd1",
            "sd2",
            "sd3",
            "hq1",
            "hq2",
            "hq3",
            "mq1",
            "mq2",
            "mq3",
            "1",
            "2",
            "3",
        ];
        let live_suffix = if live_status == Some("is_live") {
            "_live"
        } else {
            ""
        };
        for name in THUMBNAIL_NAMES {
            for extension in ["webp", "jpg"] {
                let webp = if extension == "webp" { "_webp" } else { "" };
                thumbnails.push(serde_json::json!({
                    "url": format!(
                        "https://i.ytimg.com/vi{webp}/{video_id}/{name}{live_suffix}.{extension}"
                    ),
                }));
            }
        }
        let mut seen_thumbnails = std::collections::BTreeSet::new();
        thumbnails.retain(|thumbnail| {
            thumbnail
                .get("url")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|url| seen_thumbnails.insert(url.to_owned()))
        });
        for thumbnail in &mut thumbnails {
            let url = thumbnail
                .get("url")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let rank = THUMBNAIL_NAMES
                .iter()
                .position(|name| url.contains(&format!("/{video_id}/{name}")))
                .unwrap_or(THUMBNAIL_NAMES.len());
            let preference = (if url.contains(".webp") { 0 } else { -1 }) - 2 * rank as i64;
            if let Some(object) = thumbnail.as_object_mut() {
                object.insert("preference".to_owned(), serde_json::json!(preference));
            }
        }
        let category = youtube_json_string(microformat, "category");
        let tags = details
            .get("keywords")
            .and_then(serde_json::Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        // `yt:stretch=` keywords rescale anamorphic videos, mirroring the
        // keyword loop in `_real_extract`.
        for keyword in &tags {
            if let Some(ratio) = keyword.strip_prefix("yt:stretch=").and_then(|ratio| {
                let (width, height) = ratio.split_once(':')?;
                let width = width.trim().parse::<f64>().ok()?;
                let height = height.trim().parse::<f64>().ok()?;
                (width > 0.0 && height > 0.0).then(|| width / height)
            }) {
                for format in &mut formats {
                    if format.get("vcodec").and_then(serde_json::Value::as_str) != Some("none") {
                        if let Some(object) = format.as_object_mut() {
                            object.insert("stretched_ratio".to_owned(), serde_json::json!(ratio));
                        }
                    }
                }
                break;
            }
        }
        let timestamp = youtube_json_string(microformat, "publishDate")
            .or_else(|| youtube_json_string(microformat, "uploadDate"))
            .and_then(parse_timestamp);
        let upload_date = youtube_json_string(microformat, "publishDate")
            .or_else(|| youtube_json_string(microformat, "uploadDate"))
            .and_then(|date| date_digits(&date));
        let uploader = youtube_json_string(details, "author")
            .or_else(|| youtube_json_string(microformat, "ownerChannelName"));
        let view_count = youtube_json_i64(details, "viewCount")
            .or_else(|| youtube_json_i64(microformat, "viewCount"))
            .or_else(|| {
                html_meta_value(&webpage, "interactionCount").and_then(|count| {
                    count
                        .chars()
                        .filter(|character| character.is_ascii_digit())
                        .collect::<String>()
                        .parse()
                        .ok()
                })
            });
        let age_limit = if microformat
            .get("isFamilySafe")
            .and_then(serde_json::Value::as_bool)
            == Some(false)
            || html_meta_value(&webpage, "isFamilyFriendly").as_deref() == Some("false")
            || html_meta_value(&webpage, "og:restrictions:age").as_deref() == Some("18+")
        {
            18
        } else {
            0
        };
        let media_type = if is_live_content == Some(true) {
            "livestream"
        } else if microformat
            .get("isShortsEligible")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            "short"
        } else {
            "video"
        };
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats.clone()));
        info.insert("webpage_url", serde_json::json!(webpage_url));
        info.insert_if_some("description", description.clone());
        info.insert_if_some("channel_id", channel_id.clone());
        info.insert_if_some(
            "channel_url",
            channel_id.map(|channel_id| format!("https://www.youtube.com/channel/{channel_id}")),
        );
        info.insert_if_some("uploader", uploader);
        info.insert_if_some("duration", duration);
        info.insert_if_some("view_count", view_count);
        info.insert_if_some("average_rating", youtube_json_f64(details, "averageRating"));
        info.insert("age_limit", serde_json::json!(age_limit));
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some("upload_date", upload_date);
        info.insert_if_some("thumbnails", (!thumbnails.is_empty()).then_some(thumbnails.clone()));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some("categories", category.map(|category| vec![category]));
        info.insert("tags", serde_json::json!(tags));
        info.insert("subtitles", subtitles);
        info.insert("automatic_captions", automatic_captions);
        info.insert("is_live", serde_json::json!(is_live == Some(true)));
        info.insert_if_some("live_status", live_status);
        info.insert("media_type", serde_json::json!(media_type));
        info.insert_if_some(
            "playable_in_embed",
            youtube_json_bool(
                player_response
                    .get("playabilityStatus")
                    .unwrap_or(&serde_json::Value::Null),
                "playableInEmbed",
            ),
        );
        if let Some(first) = formats.first() {
            info.insert_if_some("url", first.get("url").cloned());
            info.insert_if_some("ext", first.get("ext").cloned());
        }
        if let Some(live_details) = live_details {
            if let Some(start) = youtube_json_string(live_details, "startTimestamp")
                .and_then(parse_timestamp)
            {
                info.insert("release_timestamp", serde_json::json!(start));
            }
        }
        // `t`/`start`/`end` URL parameters clip playback, from either the
        // fragment or the query string.
        if let Ok(parsed) = url::Url::parse(url) {
            let mut start_time = None;
            let mut end_time = None;
            let fragment = parsed.fragment().unwrap_or_default();
            let query = parsed.query().unwrap_or_default();
            for component in [fragment, query] {
                for (key, value) in url::form_urlencoded::parse(component.as_bytes()) {
                    if start_time.is_none() && (key == "t" || key == "start") {
                        start_time = yt_dlp_core::parse_duration(&value);
                    } else if end_time.is_none() && key == "end" {
                        end_time = yt_dlp_core::parse_duration(&value);
                    }
                }
            }
            info.insert_if_some("start_time", start_time);
            info.insert_if_some("end_time", end_time);
        }
        youtube_music_metadata(&mut info, description.as_deref().unwrap_or_default());
        // Name the concrete player revision in leftover solver TODOs, per the
        // readiness gate; a failed inventory keeps the generic TODOs.
        todos = youtube_annotate_challenge_todos(todos, player.as_ref());
        todos.sort();
        todos.dedup();
        if !todos.is_empty() {
            info.insert("rust_todo", serde_json::json!(todos));
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Mirrors the YouTube Music auto-generated description parsing in
/// `_real_extract`: track, artist, album, and release metadata.
pub(crate) fn youtube_music_metadata(info: &mut InfoDict, description: &str) {
    if !description
        .trim_end()
        .ends_with("\nAuto-generated by YouTube.")
    {
        return;
    }
    let pattern = concat!(
        r#"(?s)(?:\n|^)(?P<track>[^\n·]+)\ ·\ (?P<artist>[^\n]+)\n+"#,
        r#"(?P<album>[^\n]+)\n+"#,
        r#"(?:℗\s*(?P<release_year>\d{4}))?"#,
        r#"(?:.+?\nReleased\ on\s*:\s*(?P<release_date>\d{4}-\d{2}-\d{2}))?"#,
        r#"(?:.+?\nArtist\s*:\s*(?P<clean_artist>[^\n]+)\n)?"#,
        r#".+\nAuto-generated\ by\ YouTube\.\s*$"#,
    );
    let captures = Regex::new(pattern)
        .ok()
        .and_then(|matcher| matcher.captures(description).ok().flatten());
    let Some(captures) = captures else {
        return;
    };
    let text = |name: &str| {
        captures
            .name(name)
            .map(|capture| capture.as_str().trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let (Some(track), Some(artist), Some(album)) = (text("track"), text("artist"), text("album"))
    else {
        return;
    };
    let artists = match text("clean_artist") {
        Some(clean) => vec![clean],
        None => artist
            .split('·')
            .map(|part| part.trim().to_owned())
            .filter(|part| !part.is_empty())
            .collect(),
    };
    let release_date = text("release_date").map(|date| date.replace('-', ""));
    let release_year = text("release_year").or_else(|| {
        release_date
            .as_deref()
            .filter(|date| date.len() >= 4)
            .map(|date| date[..4].to_owned())
    });
    info.insert("track", serde_json::json!(track.clone()));
    info.insert("alt_title", serde_json::json!(track));
    info.insert("album", serde_json::json!(album));
    if !artists.is_empty() {
        info.insert("artists", serde_json::json!(artists.clone()));
        info.insert("creators", serde_json::json!(artists));
    }
    info.insert_if_some("release_date", release_date);
    info.insert_if_some(
        "release_year",
        release_year.and_then(|year| year.parse::<i64>().ok()),
    );
}

impl InfoExtractor for YoutubeExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        youtube_video_id(url).is_some()
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
        self.extract_native(url, context)
    }

    fn extract(&self, _url: &str) -> Result<InfoDict, ExtractorError> {
        Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            "TODO: native YouTube extraction requires a request context",
        ))
    }
}
