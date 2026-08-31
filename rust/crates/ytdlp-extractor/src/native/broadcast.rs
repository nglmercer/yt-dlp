/// Native CozyTV replay extractor.
pub struct CozyTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CozyTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CozyTvExtractor {
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
                "CozyTV URL did not match its native pattern",
            )
        })?;
        let uploader = captures
            .name("uploader")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CozyTV URL has no uploader")
            })?;
        let date = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "CozyTV URL has no replay ID",
                )
            })?;
        let video_id = format!("{uploader}-{date}");
        let data = context.get_json(&format!(
            "https://api.cozy.tv/cache/{uploader}/replay/{date}"
        ))?;
        let media_url =
            format!("https://cozycdn.foxtrotstream.xyz/replays/{uploader}/{date}/index.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert(
            "uploader",
            serde_json::json!(json_string(&data, "user").unwrap_or(&uploader)),
        );
        info.insert(
            "upload_date",
            serde_json::json!(
                json_string(&data, "date")
                    .map(|value| {
                        value
                            .chars()
                            .filter(|character| character.is_ascii_digit())
                            .take(8)
                            .collect::<String>()
                    })
                    .filter(|value| value.len() == 8)
                    .unwrap_or_else(|| {
                        date.chars()
                            .filter(|character| character.is_ascii_digit())
                            .take(8)
                            .collect::<String>()
                    })
            ),
        );
        info.insert("was_live", serde_json::json!(true));
        info.insert_if_some("duration", json_i64(&data, "duration"));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Livestreamfails API/direct-media extractor.
pub struct LivestreamfailsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LivestreamfailsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LivestreamfailsExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Livestreamfails URL has no ID",
                )
            })?;
        let data = context.get_json(&format!("https://api.livestreamfails.com/clip/{video_id}"))?;
        let source_id = json_string(&data, "sourceId").map(str::to_owned);
        let remote_id = json_string(&data, "videoId").unwrap_or(&video_id);
        let media_url = format!("https://livestreamfails-video-prod.b-cdn.net/video/{remote_id}");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("display_id", source_id);
        info.insert_if_some("title", json_string(&data, "label").map(str::to_owned));
        info.insert_if_some(
            "creator",
            data.get("streamer")
                .and_then(|value| json_string(value, "label")),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&data, "imageId")
                .map(|value| format!("https://livestreamfails-image-prod.b-cdn.net/image/{value}")),
        );
        info.insert_if_some(
            "timestamp",
            json_string(&data, "createdAt")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Masters tournament video API extractor.
pub struct MastersExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MastersExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MastersExtractor {
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
                "Masters URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Masters URL has no ID")
            })?;
        let date = captures
            .name("date")
            .map(|value| value.as_str().replace('-', ""))
            .unwrap_or_default();
        let data = context.get_json(&format!(
            "https://www.masters.com/relatedcontent/rest/v2/masters_v1/en/content/masters_v1_{video_id}_en"
        ))?;
        let media_url = data
            .get("media")
            .and_then(|value| json_string(value, "m3u8"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Masters video {video_id} has no HLS URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert("upload_date", serde_json::json!(date));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        if let Some(images) = data
            .get("images")
            .and_then(|value| value.get(0))
            .and_then(serde_json::Value::as_object)
        {
            let thumbnails = images
                .iter()
                .filter_map(|(id, value)| {
                    Some(serde_json::json!({
                        "id": id,
                        "url": value.as_str()?
                    }))
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Mir24 article iframe/HLS extractor.
pub struct Mir24TvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Mir24TvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Mir24TvExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mir24 URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let iframe_url =
            Regex::new(r#"(?is)<iframe\b[^>]+\bsrc\s*=\s*["'](https?://mir24\.tv/players/[^"']+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Mir24 article {video_id} has no player iframe"),
                    )
                })?;
        let player = url::Url::parse(&iframe_url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Mir24 player URL: {error}"),
            )
        })?;
        let media_url = player
            .query_pairs()
            .find_map(|(key, value)| {
                (key == "source").then(|| proto_relative_url(value.as_ref(), "https:"))
            })
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Mir24 player {video_id} has no source URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title")
                    .or_else(|| html_title_value(&html))
                    .unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Blogger video configuration extractor.
pub struct BloggerExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BloggerExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BloggerExtractor {
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
        let token_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Blogger URL has no token")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let data = json_object_after_marker(&html, "var VIDEO_CONFIG").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Blogger video {token_id} has no VIDEO_CONFIG object"),
            )
        })?;
        let streams = data
            .get("streams")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Blogger video {token_id} has no streams"),
                )
            })?;
        let mut formats = Vec::new();
        for stream in streams {
            let Some(play_url) = json_string(stream, "play_url") else {
                continue;
            };
            let ext = url_query_value(play_url, "mime")
                .and_then(|mime| mimetype_extension(Some(&mime)))
                .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(play_url), "mp4"));
            formats.push(serde_json::json!({
                "url": play_url,
                "format_id": json_value_string(stream.get("format_id")),
                "ext": ext,
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Blogger video {token_id} has no playable streams"),
            )
        })?;
        let video_id = json_string(&data, "iframe_id").unwrap_or(&token_id);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(video_id));
        info.insert_if_some("thumbnail", json_string(&data, "thumbnail"));
        info.insert_if_some(
            "duration",
            first
                .get("url")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| url_query_value(value, "dur"))
                .and_then(|value| yt_dlp_core::parse_duration(&value)),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native radio.de station-page extractor. The source marks this service as
/// non-working today, but its historical contract is still represented here
/// without a compatibility runtime.
pub struct RadioDeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RadioDeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RadioDeExtractor {
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
        let radio_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "radio.de URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let broadcast = json_object_after_marker(&html, "stationService")
            .and_then(|service| service.get("station").cloned())
            .or_else(|| json_object_after_marker(&html, "station"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("radio.de station {radio_id} has no broadcast data"),
                )
            })?;
        let stream_urls = broadcast
            .get("streamUrls")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("radio.de station {radio_id} has no streams"),
                )
            })?;
        let mut formats = Vec::new();
        for stream in stream_urls {
            let Some(stream_url) = json_string(stream, "streamUrl") else {
                continue;
            };
            let codec = json_string(stream, "streamContentFormat").unwrap_or("mp3");
            formats.push(serde_json::json!({
                "url": stream_url,
                "ext": codec.to_ascii_lowercase(),
                "acodec": codec,
                "abr": json_f64(stream, "bitRate"),
                "asr": json_f64(stream, "sampleRate"),
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("radio.de station {radio_id} has no playable streams"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(radio_id));
        info.insert(
            "title",
            serde_json::json!(json_string(&broadcast, "name").unwrap_or("radio.de station")),
        );
        info.insert_if_some(
            "description",
            json_string(&broadcast, "description")
                .or_else(|| json_string(&broadcast, "shortDescription")),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&broadcast, "picture4Url")
                .or_else(|| json_string(&broadcast, "picture4TransUrl"))
                .or_else(|| json_string(&broadcast, "logo100x100")),
        );
        info.insert("is_live", serde_json::json!(true));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native RadioZET podcast API extractor.
pub struct RadioZetPodcastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RadioZetPodcastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RadioZetPodcastExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "RadioZET URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let podcast_id = Regex::new(
            r#"(?is)<div\b[^>]*\bid\s*=\s*["']player["'][^>]*\bdata-id\s*=\s*["']([^"']+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("RadioZET podcast {display_id} has no player ID"),
            )
        })?;
        let data_url = format!(
            "https://player.radiozet.pl/api/podcasts/getPodcast/(node)/{podcast_id}/(station)/radiozet"
        );
        let response = context.get_json(&data_url)?;
        let data = response
            .get("data")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("RadioZET podcast {podcast_id} has no API record"),
                )
            })?;
        let stream_url = data
            .get("player")
            .and_then(|player| json_string(player, "stream"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("RadioZET podcast {podcast_id} has no audio stream"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(podcast_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some(
            "title",
            data.get("title")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "description",
            data.get("program")
                .and_then(|program| json_string(program, "desc"))
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "release_timestamp",
            data.get("published_date").and_then(|value| {
                value
                    .as_i64()
                    .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
            }),
        );
        info.insert_if_some(
            "thumbnail",
            data.get("program")
                .and_then(|program| program.get("image"))
                .and_then(|image| json_string(image, "original")),
        );
        info.insert_if_some(
            "duration",
            data.get("player")
                .and_then(|player| player.get("duration"))
                .cloned(),
        );
        info.insert_if_some(
            "series",
            data.get("program")
                .and_then(|program| json_string(program, "title"))
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "creator",
            data.get("presenter")
                .and_then(serde_json::Value::as_array)
                .and_then(|presenters| presenters.first())
                .and_then(|presenter| json_string(presenter, "title"))
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        let ext = yt_dlp_core::determine_ext(Some(stream_url), "mp3");
        info.insert("url", serde_json::json!(stream_url));
        info.insert("ext", serde_json::json!(ext));
        info.insert(
            "formats",
            serde_json::json!([{"url": stream_url, "format_id": "source", "ext": ext}]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native WorldStarHipHop HTML5 media extractor.
pub struct WorldStarHipHopExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WorldStarHipHopExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WorldStarHipHopExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "WorldStar URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(r#"(?is)<div\b[^>]*class\s*=\s*["'][^"']*content-heading[^"']*["'][^>]*>\s*<h1\b[^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                Regex::new(r#"(?is)<span\b[^>]*class\s*=\s*["'][^"']*tc-sp-pinned-title[^"']*["'][^>]*>(.*?)</span>"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&html).ok().flatten())
                    .and_then(|captures| captures.get(1))
                    .map(|value| html_text_fragment(value.as_str()))
                    .filter(|value| !value.trim().is_empty())
            })
            .or_else(|| html_meta_value(&html, "og:title"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("WorldStar video {video_id} has no title"),
                )
            })?;
        let formats = html5_media_formats(url, &html);
        if formats.is_empty() {
            let generic =
                GenericExtractor::new(ExtractorDescriptor::new("GenericIE", "Generic", "", true));
            let fallback = generic.extract_with_context(url, context)?;
            if let ExtractorResult::Single(mut info) = fallback {
                info.insert("id", serde_json::json!(video_id));
                info.insert("title", serde_json::json!(title));
                return Ok(ExtractorResult::single(info));
            }
            return Ok(fallback);
        }
        let first = formats.first().cloned().expect("WorldStar format");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native This American Life archive/audio extractor.
pub struct ThisAmericanLifeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ThisAmericanLifeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ThisAmericanLifeExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "This American Life URL has no ID",
                )
            })?;
        let page_url = format!("http://www.thisamericanlife.org/radio-archives/episode/{video_id}");
        let webpage = context.get(&page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let stream_url =
            format!("http://stream.thisamericanlife.org/{video_id}/stream/{video_id}_64k.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("protocol", serde_json::json!("m3u8_native"));
        info.insert("ext", serde_json::json!("m4a"));
        info.insert("acodec", serde_json::json!("aac"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert("abr", serde_json::json!(64));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "twitter:title").unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "m4a",
                "acodec": "aac",
                "vcodec": "none",
                "abr": 64,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
