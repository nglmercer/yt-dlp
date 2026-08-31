/// Native Bandcamp track extractor. Track metadata and playable encodings are
/// read from the page's tralbum/embed JSON attributes without executing the
/// Bandcamp player.
pub struct BandcampTrackExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BandcampTrackExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BandcampTrackExtractor {
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
                "Bandcamp track URL did not match its native pattern",
            )
        })?;
        let uploader = captures
            .name("uploader")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Bandcamp URL has no uploader",
                )
            })?;
        let page_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Bandcamp URL has no track slug",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let tralbum = html_data_json_attribute(&html, "tralbum").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Bandcamp page has no tralbum JSON",
            )
        })?;
        let track_info = tralbum
            .get("trackinfo")
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Bandcamp page has no track information",
                )
            })?;
        let mut formats = Vec::new();
        if let Some(files) = track_info
            .get("file")
            .and_then(serde_json::Value::as_object)
        {
            for (format_id, value) in files {
                let Some(raw_url) = value.as_str() else {
                    continue;
                };
                let Some((extension, bitrate)) = format_id.split_once('-') else {
                    continue;
                };
                let media_url = raw_url
                    .strip_prefix("//")
                    .map_or_else(|| raw_url.to_owned(), |url| format!("https://{url}"));
                let mut format = serde_json::json!({
                    "format_id": format_id,
                    "url": media_url,
                    "ext": extension,
                    "vcodec": "none",
                    "acodec": extension,
                });
                if let Ok(bitrate) = bitrate.parse::<i64>() {
                    format["abr"] = serde_json::json!(bitrate);
                }
                formats.push(format);
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Bandcamp track {page_id} has no playable encodings"),
            ));
        }
        let embed = html_data_json_attribute(&html, "embed").unwrap_or(serde_json::Value::Null);
        let current = tralbum.get("current").unwrap_or(&serde_json::Value::Null);
        let track = json_string(track_info, "title").map(str::to_owned);
        let artist = json_string(&embed, "artist")
            .or_else(|| json_string(current, "artist"))
            .or_else(|| json_string(&tralbum, "artist"))
            .map(str::to_owned);
        let title = match (artist.as_deref(), track.as_deref()) {
            (Some(artist), Some(track)) => format!("{artist} - {track}"),
            (None, Some(track)) => track.to_owned(),
            (_, None) => page_id.to_owned(),
        };
        let track_id =
            json_value_string(track_info.get("track_id").or_else(|| track_info.get("id")))
                .or_else(|| json_value_string(tralbum.get("id")))
                .unwrap_or_else(|| page_id.to_owned());
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id.clone()));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp3")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("track", track);
        info.insert_if_some("artist", artist.clone());
        info.insert_if_some("uploader", artist);
        info.insert("uploader_id", serde_json::json!(uploader));
        info.insert(
            "uploader_url",
            serde_json::json!(format!("https://{uploader}.bandcamp.com")),
        );
        info.insert_if_some("album", json_string(&embed, "album_title"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some("duration", json_f64(track_info, "duration"));
        info.insert_if_some("track_number", json_i64(track_info, "track_num"));
        info.insert("track_id", serde_json::json!(track_id));
        if let Ok(tag_matcher) = Regex::new(
            r#"(?is)<(?:a|span)\b[^>]*class\s*=\s*["'][^"']*\btag\b[^"']*["'][^>]*>(.*?)</(?:a|span)>"#,
        ) {
            let tags = tag_matcher
                .captures_iter(&html)
                .flatten()
                .filter_map(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                })
                .filter(|tag| !tag.is_empty())
                .map(serde_json::Value::String)
                .collect::<Vec<_>>();
            if !tags.is_empty() {
                info.insert("tags", serde_json::Value::Array(tags));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

const BANNED_VIDEO_QUERY: &str = r#"
query GetVideoAndComments($id: String!) {
    getVideo(id: $id) {
        streamUrl
        directUrl
        unlisted
        live
        tags { name }
        title
        summary
        playCount
        largeImage
        videoDuration
        channel { _id title }
        createdAt
    }
    getVideoComments(id: $id, limit: 999999, offset: 0) {
        _id
        content
        user { _id username }
        voteCount { positive }
        createdAt
        replyCount
    }
}"#;

const BANNED_COMMENT_REPLIES_QUERY: &str = r#"
query GetCommentReplies($id: String!) {
    getCommentReplies(id: $id, limit: 999999, offset: 0) {
        _id
        content
        user { _id username }
        voteCount { positive }
        createdAt
        replyCount
    }
}"#;

fn banned_video_call(
    context: &ExtractionContext,
    id: &str,
    operation: &str,
    query: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let response = native_post_json(
        context,
        "https://api.infowarsmedia.com/graphql",
        &serde_json::json!({
            "variables": {"id": id},
            "operationName": operation,
            "query": query,
        }),
    )?;
    response.get("data").cloned().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "BannedVideo GraphQL response has no data",
        )
    })
}

fn banned_comment_value(comment: &serde_json::Value, parent: &str) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    value.insert(
        "id".to_owned(),
        json_value_string(comment.get("_id"))
            .map_or(serde_json::Value::Null, serde_json::Value::String),
    );
    value.insert(
        "text".to_owned(),
        json_string(comment, "content")
            .map_or(serde_json::Value::Null, |text| serde_json::json!(text)),
    );
    if let Some(user) = comment.get("user") {
        if let Some(username) = json_string(user, "username") {
            value.insert("author".to_owned(), serde_json::json!(username));
        }
        if let Some(user_id) = json_value_string(user.get("_id")) {
            value.insert("author_id".to_owned(), serde_json::json!(user_id));
        }
    }
    if let Some(timestamp) = comment.get("createdAt") {
        value.insert("timestamp".to_owned(), timestamp.clone());
    }
    value.insert("parent".to_owned(), serde_json::json!(parent));
    if let Some(likes) = comment
        .get("voteCount")
        .and_then(|votes| json_i64(votes, "positive"))
    {
        value.insert("like_count".to_owned(), serde_json::json!(likes));
    }
    serde_json::Value::Object(value)
}

/// Native BannedVideo GraphQL extractor. Metadata, media variants, and
/// available comments are fetched with typed Rust requests and no scripting
/// runtime.
pub struct BannedVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BannedVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BannedVideoExtractor {
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
                "BannedVideo URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "BannedVideo URL has no ID")
            })?;
        let data = banned_video_call(context, video_id, "GetVideoAndComments", BANNED_VIDEO_QUERY)?;
        let video = data.get("getVideo").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "BannedVideo response has no video object",
            )
        })?;
        let mut formats = Vec::new();
        if let Some(media_url) = json_string(video, "directUrl") {
            formats.push(serde_json::json!({
                "format_id": "direct",
                "quality": 1,
                "url": media_url,
                "ext": "mp4",
                "protocol": "http",
            }));
        }
        if let Some(media_url) = json_string(video, "streamUrl") {
            formats.push(serde_json::json!({
                "format_id": "hls",
                "url": media_url,
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "BannedVideo response has no playable media URLs",
            ));
        }
        let title = json_string(video, "title")
            .map(|title| title.strip_suffix('.').unwrap_or(title).to_owned())
            .unwrap_or_else(|| video_id.to_owned());
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some(
            "is_live",
            video.get("live").and_then(serde_json::Value::as_bool),
        );
        info.insert_if_some("description", json_string(video, "summary"));
        if let Some(channel) = video.get("channel") {
            info.insert_if_some("channel", json_string(channel, "title"));
            info.insert_if_some("channel_id", json_value_string(channel.get("_id")));
        }
        info.insert_if_some("view_count", json_i64(video, "playCount"));
        info.insert_if_some("thumbnail", json_string(video, "largeImage"));
        info.insert_if_some("duration", json_f64(video, "videoDuration"));
        if let Some(tags) = video.get("tags").and_then(serde_json::Value::as_array) {
            let tags = tags
                .iter()
                .filter_map(|tag| json_string(tag, "name"))
                .map(|tag| serde_json::json!(tag))
                .collect::<Vec<_>>();
            info.insert("tags", serde_json::Value::Array(tags));
        }
        if let Some(comments) = data
            .get("getVideoComments")
            .and_then(serde_json::Value::as_array)
        {
            let mut all_comments = Vec::new();
            for comment in comments {
                let comment_id = json_value_string(comment.get("_id")).unwrap_or_default();
                all_comments.push(banned_comment_value(comment, "root"));
                if json_i64(comment, "replyCount").unwrap_or_default() > 0 && !comment_id.is_empty()
                {
                    if let Ok(reply_data) = banned_video_call(
                        context,
                        &comment_id,
                        "GetCommentReplies",
                        BANNED_COMMENT_REPLIES_QUERY,
                    ) {
                        if let Some(replies) = reply_data
                            .get("getCommentReplies")
                            .and_then(serde_json::Value::as_array)
                        {
                            all_comments.extend(
                                replies
                                    .iter()
                                    .map(|reply| banned_comment_value(reply, &comment_id)),
                            );
                        }
                    }
                }
            }
            info.insert("comments", serde_json::Value::Array(all_comments));
        }
        Ok(ExtractorResult::single(info))
    }
}
