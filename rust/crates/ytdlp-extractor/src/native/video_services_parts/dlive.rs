/// Native DLive GraphQL VOD/live-stream extractors.
pub struct DliveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DliveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DliveExtractor {
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
                "DLive URL did not match its native pattern",
            )
        })?;
        if self.descriptor.key == "DLiveVODIE" {
            let uploader_id = captures
                .name("uploader_id")
                .map(|value| value.as_str().to_owned())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::InvalidUrl,
                        "DLive VOD URL has no uploader ID",
                    )
                })?;
            let video_id = captures
                .name("id")
                .map(|value| value.as_str().to_owned())
                .ok_or_else(|| {
                    ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DLive VOD URL has no ID")
                })?;
            let query = format!(
                "query {{ pastBroadcast(permlink:\"{uploader_id}+{video_id}\") \
                 {{ content createdAt length playbackUrl title thumbnailUrl viewCount }} }}"
            );
            let data = native_post_json(
                context,
                "https://graphigo.prd.dlive.tv/",
                &serde_json::json!({ "query": query }),
            )?;
            let broadcast = data
                .get("data")
                .and_then(|data| data.get("pastBroadcast"))
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("DLive VOD {video_id} has no past broadcast"),
                    )
                })?;
            let playback_url = json_string(broadcast, "playbackUrl")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("DLive VOD {video_id} has no playback URL"),
                    )
                })?;
            let title = json_string(broadcast, "title")
                .filter(|value| !value.is_empty())
                .unwrap_or(&video_id);
            return Ok(ExtractorResult::single(dlive_media_info(
                &video_id,
                title,
                playback_url,
                json_string(broadcast, "content"),
                json_string(broadcast, "thumbnailUrl"),
                Some(uploader_id),
                None,
                json_i64(broadcast, "createdAt").map(|value| value / 1000),
                json_i64(broadcast, "viewCount"),
                false,
            )));
        }

        let display_name = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "DLive stream URL has no display name",
                )
            })?;
        let query = format!(
            "query {{ userByDisplayName(displayname:\"{display_name}\") \
             {{ livestream {{ content createdAt title thumbnailUrl watchingCount }} username }} }}"
        );
        let data = native_post_json(
            context,
            "https://graphigo.prd.dlive.tv/",
            &serde_json::json!({ "query": query }),
        )?;
        let user = data
            .get("data")
            .and_then(|data| data.get("userByDisplayName"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("DLive user {display_name} was not found"),
                )
            })?;
        let livestream = user.get("livestream").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DLive user {display_name} has no livestream"),
            )
        })?;
        let username = json_string(user, "username")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("DLive user {display_name} has no username"),
                )
            })?;
        let playback_url = format!("https://live.prd.dlive.tv/hls/live/{username}.m3u8");
        let title = json_string(livestream, "title")
            .filter(|value| !value.is_empty())
            .unwrap_or(&display_name);
        Ok(ExtractorResult::single(dlive_media_info(
            &display_name,
            title,
            &playback_url,
            json_string(livestream, "content"),
            json_string(livestream, "thumbnailUrl"),
            Some(username.to_owned()),
            Some(display_name.clone()),
            json_i64(livestream, "createdAt").map(|value| value / 1000),
            json_i64(livestream, "watchingCount"),
            true,
        )))
    }
}

fn dlive_media_info(
    video_id: &str,
    title: &str,
    playback_url: &str,
    description: Option<&str>,
    thumbnail: Option<&str>,
    uploader_id: Option<String>,
    uploader: Option<String>,
    timestamp: Option<i64>,
    view_count: Option<i64>,
    is_live: bool,
) -> InfoDict {
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert("title", serde_json::json!(title));
    info.insert_if_some("description", description);
    info.insert_if_some("thumbnail", thumbnail);
    info.insert_if_some("uploader_id", uploader_id);
    info.insert_if_some("uploader", uploader);
    info.insert_if_some("timestamp", timestamp);
    info.insert_if_some("view_count", view_count);
    info.insert("url", serde_json::json!(playback_url));
    info.insert("ext", serde_json::json!("mp4"));
    info.insert(
        "formats",
        serde_json::json!([{
            "url": playback_url,
            "format_id": "hls",
            "protocol": "m3u8_native",
            "ext": "mp4",
        }]),
    );
    info.insert("subtitles", serde_json::json!({}));
    if is_live {
        info.insert("is_live", serde_json::json!(true));
    }
    info
}
