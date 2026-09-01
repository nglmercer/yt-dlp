/// Native FourTube-family metadata/token extractor.
pub struct FourTubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FourTubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FourTubeExtractor {
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
                "FourTube-family URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "FourTube-family URL has no video ID",
                )
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned());
        let kind = captures.name("kind").map(|value| value.as_str());
        let (token_host, canonical_url, is_porn_tube) = fourtube_site(&self.descriptor.key)?;
        let page_url = if kind == Some("m") || display_id.is_none() {
            canonical_url.replace("{id}", &video_id)
        } else {
            url.to_owned()
        };
        let response = context.get(&page_url)?;
        let webpage = String::from_utf8_lossy(response.body());

        let mut metadata = FourTubeMetadata::default();
        if is_porn_tube {
            let video = fourtube_porn_tube_video(&webpage, &video_id)?;
            metadata.title = json_string(&video, "title").map(str::to_owned);
            metadata.thumbnail = json_string(&video, "masterThumb").map(str::to_owned);
            metadata.uploader = video
                .get("user")
                .and_then(|user| json_string(user, "username"))
                .map(str::to_owned)
                .or_else(|| {
                    video
                        .get("channel")
                        .and_then(|channel| json_string(channel, "name"))
                        .map(str::to_owned)
                });
            metadata.uploader_id = video
                .get("user")
                .and_then(|user| json_value_string(user.get("id")))
                .or_else(|| {
                    video
                        .get("channel")
                        .and_then(|channel| json_value_string(channel.get("id")))
                });
            metadata.channel = video
                .get("channel")
                .and_then(|channel| json_string(channel, "name"))
                .map(str::to_owned);
            metadata.channel_id = video
                .get("channel")
                .and_then(|channel| json_value_string(channel.get("id")));
            metadata.like_count = json_i64(&video, "likes");
            metadata.dislike_count = json_i64(&video, "dislikes");
            metadata.view_count = json_i64(&video, "playsQty");
            metadata.duration = json_f64(&video, "durationInSeconds");
            metadata.timestamp = json_string(&video, "publishedAt")
                .map(str::to_owned)
                .and_then(parse_timestamp);
            metadata.media_id = json_value_string(video.get("mediaId"));
            metadata.sources = video
                .get("encodings")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|encoding| {
                    json_value_string(encoding.get("height"))
                        .filter(|height| !height.is_empty())
                })
                .collect();
        } else {
            metadata.title = html_meta_value(&webpage, "name");
            metadata.timestamp = html_meta_value(&webpage, "uploadDate")
                .and_then(parse_timestamp);
            metadata.thumbnail = html_meta_value(&webpage, "thumbnailUrl");
            let (uploader_id, uploader) = fourtube_uploader(&webpage);
            metadata.uploader_id = uploader_id;
            metadata.uploader = uploader;
            metadata.categories = fourtube_categories(&webpage);
            metadata.view_count = fourtube_interaction_count(&webpage, "UserPlays");
            metadata.like_count = fourtube_interaction_count(&webpage, "UserLikes");
            metadata.duration = html_meta_value(&webpage, "duration")
                .and_then(|value| yt_dlp_core::parse_duration(&value));
            metadata.media_id = fourtube_media_id(&webpage);
            metadata.sources = fourtube_sources(&webpage);
        }

        let title = metadata.title.ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FourTube-family video {video_id} has no title"),
            )
        })?;
        if metadata.media_id.is_none() || metadata.sources.is_empty() {
            let (media_id, sources) =
                fourtube_player_parameters(&webpage, &page_url, &video_id, context)?;
            metadata.media_id = metadata.media_id.or(Some(media_id));
            if metadata.sources.is_empty() {
                metadata.sources = sources;
            }
        }
        let media_id = metadata.media_id.ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: FourTube-family video {video_id} has no native media ID; \
                     player bootstrap format is not implemented"
                ),
            )
        })?;
        if metadata.sources.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: FourTube-family video {video_id} has no native quality list; \
                     player bootstrap format is not implemented"
                ),
            ));
        }
        let formats = fourtube_token_formats(
            context,
            &page_url,
            &video_id,
            token_host,
            &media_id,
            &metadata.sources,
        )?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("thumbnail", metadata.thumbnail);
        info.insert_if_some("uploader", metadata.uploader);
        info.insert_if_some("uploader_id", metadata.uploader_id);
        info.insert_if_some("channel", metadata.channel);
        info.insert_if_some("channel_id", metadata.channel_id);
        info.insert_if_some("timestamp", metadata.timestamp);
        info.insert_if_some(
            "upload_date",
            metadata
                .timestamp
                .and_then(chrono_like_date_digits),
        );
        info.insert_if_some("duration", metadata.duration);
        info.insert_if_some("like_count", metadata.like_count);
        info.insert_if_some("dislike_count", metadata.dislike_count);
        info.insert_if_some("view_count", metadata.view_count);
        info.insert_if_some("categories", metadata.categories);
        info.insert("age_limit", serde_json::json!(18));
        Ok(ExtractorResult::single(info))
    }
}

#[derive(Default)]
struct FourTubeMetadata {
    title: Option<String>,
    thumbnail: Option<String>,
    uploader: Option<String>,
    uploader_id: Option<String>,
    channel: Option<String>,
    channel_id: Option<String>,
    timestamp: Option<i64>,
    duration: Option<f64>,
    like_count: Option<i64>,
    dislike_count: Option<i64>,
    view_count: Option<i64>,
    categories: Option<Vec<String>>,
    media_id: Option<String>,
    sources: Vec<String>,
}

fn fourtube_site(key: &str) -> Result<(&'static str, &'static str, bool), ExtractorError> {
    match key {
        "FourTubeIE" => Ok((
            "token.4tube.com",
            "https://www.4tube.com/videos/{id}/video",
            false,
        )),
        "FuxIE" => Ok(("token.fux.com", "https://www.fux.com/video/{id}/video", false)),
        "PornTubeIE" => Ok((
            "tkn.porntube.com",
            "https://www.porntube.com/videos/video_{id}",
            true,
        )),
        "PornerBrosIE" => Ok((
            "token.pornerbros.com",
            "https://www.pornerbros.com/videos/video_{id}",
            false,
        )),
        _ => Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: unsupported FourTube-family descriptor {key}"),
        )),
    }
}
