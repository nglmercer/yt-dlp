/// Native standard YouTube video extractor.
///
/// This first native boundary intentionally covers video URLs only. Playlist,
/// search, feed, clip, and account URLs retain their generated descriptors and
/// remain explicit TODOs until their continuation APIs are ported.
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
    ) -> Result<InfoDict, ExtractorError> {
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
            youtube_api_response(context, &ytcfg, &video_id).or_else(|error| {
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
        let (formats, mut todos) = youtube_formats_and_todos(&responses);
        if formats.is_empty() {
            let reason = player_response
                .get("playabilityStatus")
                .and_then(|status| youtube_json_string(status, "reason"))
                .unwrap_or_else(|| "YouTube returned no downloadable formats".to_owned());
            let message = if todos.is_empty() {
                format!("YouTube video {video_id}: {reason}")
            } else {
                todos.join("; ")
            };
            return Err(ExtractorError::new(ExtractorErrorKind::Unsupported, message));
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
        let duration = youtube_json_i64(details, "lengthSeconds")
            .or_else(|| youtube_json_i64(microformat, "lengthSeconds"));
        let is_live = youtube_json_bool(details, "isLive").unwrap_or(false);
        let is_live_content = youtube_json_bool(details, "isLiveContent").unwrap_or(false);
        let is_upcoming = youtube_json_bool(details, "isUpcoming").unwrap_or(false);
        let is_post_live = youtube_json_bool(details, "isPostLiveDvr").unwrap_or(false);
        let live_status = if is_post_live {
            Some("post_live")
        } else if is_live {
            Some("is_live")
        } else if is_upcoming {
            Some("is_upcoming")
        } else if is_live_content {
            Some("was_live")
        } else {
            Some("not_live")
        };
        let (subtitles, automatic_captions) = youtube_caption_entries(&player_response);
        let thumbnails = details
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
        let thumbnail = thumbnails
            .iter()
            .filter_map(|thumbnail| {
                let width = thumbnail.get("width").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let height = thumbnail.get("height").and_then(serde_json::Value::as_i64).unwrap_or(0);
                Some((width.saturating_mul(height), thumbnail.get("url")?.clone()))
            })
            .max_by_key(|(area, _)| *area)
            .map(|(_, url)| url);
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
        let timestamp = youtube_json_string(microformat, "publishDate")
            .or_else(|| youtube_json_string(microformat, "uploadDate"))
            .and_then(parse_timestamp);
        let upload_date = youtube_json_string(microformat, "publishDate")
            .or_else(|| youtube_json_string(microformat, "uploadDate"))
            .and_then(|date| date_digits(&date));
        let uploader = youtube_json_string(details, "author")
            .or_else(|| youtube_json_string(microformat, "ownerChannelName"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats.clone()));
        info.insert("webpage_url", serde_json::json!(webpage_url));
        info.insert_if_some("description", description);
        info.insert_if_some("channel_id", channel_id.clone());
        info.insert_if_some(
            "channel_url",
            channel_id.map(|channel_id| format!("https://www.youtube.com/channel/{channel_id}")),
        );
        info.insert_if_some("uploader", uploader);
        info.insert_if_some("duration", duration);
        info.insert_if_some("view_count", youtube_json_i64(details, "viewCount"));
        info.insert_if_some("average_rating", youtube_json_f64(details, "averageRating"));
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some("upload_date", upload_date);
        info.insert_if_some("thumbnails", (!thumbnails.is_empty()).then_some(thumbnails.clone()));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some("categories", category.map(|category| vec![category]));
        info.insert("tags", serde_json::json!(tags));
        info.insert("subtitles", subtitles);
        info.insert("automatic_captions", automatic_captions);
        info.insert("is_live", serde_json::json!(is_live));
        info.insert_if_some("live_status", live_status);
        info.insert(
            "media_type",
            serde_json::json!(if is_live_content { "livestream" } else { "video" }),
        );
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
        if let Some(live_details) = microformat.get("liveBroadcastDetails") {
            if let Some(start) = youtube_json_string(live_details, "startTimestamp")
                .and_then(parse_timestamp)
            {
                info.insert("release_timestamp", serde_json::json!(start));
            }
        }
        todos.sort();
        todos.dedup();
        if !todos.is_empty() {
            info.insert("rust_todo", serde_json::json!(todos));
        }
        Ok(info)
    }
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
        self.extract_native(url, context).map(ExtractorResult::single)
    }

    fn extract(&self, _url: &str) -> Result<InfoDict, ExtractorError> {
        Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            "TODO: native YouTube extraction requires a request context",
        ))
    }
}
