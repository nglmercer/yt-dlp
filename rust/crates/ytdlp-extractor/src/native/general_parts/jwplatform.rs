/// Native JWPlatform extractor. JWPlatform's v2 endpoint is a stable JSON
/// contract used by the service's player URLs and by several site-specific
/// wrapper extractors.
pub struct JwPlatformExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

impl JwPlatformExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let matchers = descriptor
            .valid_urls
            .iter()
            .map(|pattern| {
                compile_source_pattern(pattern).map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("invalid JWPlatform URL pattern: {error}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            descriptor,
            matchers,
        })
    }
}

impl InfoExtractor for JwPlatformExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matchers
            .iter()
            .find_map(|matcher| {
                matcher
                    .captures(url)
                    .ok()
                    .flatten()
                    .and_then(|captures| captures.name("id"))
                    .map(|value| value.as_str().to_owned())
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "JWPlatform URL did not contain a media ID",
                )
            })?;
        let response =
            context.get_json(&format!("https://cdn.jwplayer.com/v2/media/{video_id}"))?;
        let base_url = format!("https://cdn.jwplayer.com/v2/media/{video_id}");
        let playlist = response
            .get("playlist")
            .and_then(serde_json::Value::as_array)
            .map(|items| items.iter().collect::<Vec<_>>())
            .unwrap_or_else(|| vec![&response]);
        let mut entries = Vec::new();
        for item in playlist {
            let item_id = json_string(item, "mediaid").unwrap_or(&video_id).to_owned();
            let sources = item
                .get("sources")
                .and_then(serde_json::Value::as_array)
                .map(|sources| sources.iter().collect::<Vec<_>>())
                .unwrap_or_else(|| vec![item]);
            let mut formats = Vec::new();
            for (index, source) in sources.into_iter().enumerate() {
                let Some(raw_url) = json_string(source, "file").filter(|value| !value.is_empty())
                else {
                    continue;
                };
                if raw_url.starts_with("rtmp:") {
                    continue;
                }
                let source_url = resolve_url(&base_url, &proto_relative_url(raw_url, "https:"));
                let source_type = json_string(source, "type").unwrap_or("");
                let source_ext = source_type
                    .split(';')
                    .next()
                    .and_then(|value| mimetype_extension(Some(value)))
                    .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(&source_url), "mp4"));
                let url_ext = yt_dlp_core::determine_ext(Some(&source_url), &source_ext);
                let is_hls = source_type.eq_ignore_ascii_case("hls")
                    || url_ext == "m3u8"
                    || source_url.contains("format=m3u8-aapl");
                let is_dash = source_type.eq_ignore_ascii_case("dash")
                    || url_ext == "mpd"
                    || source_url.contains("format=mpd-time-csf");
                let format_id = json_string(source, "label")
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("http-{index}"));
                let mut format = serde_json::json!({
                    "url": source_url,
                    "format_id": format_id,
                    "ext": if is_hls || is_dash { "mp4" } else { url_ext.as_str() },
                });
                if is_hls {
                    format["protocol"] = serde_json::json!("m3u8_native");
                } else if is_dash {
                    format["protocol"] = serde_json::json!("http_dash_segments");
                } else if source_type.starts_with("audio/")
                    || source_ext.starts_with("mp3")
                    || matches!(url_ext.as_str(), "mp3" | "m4a" | "aac" | "ogg" | "oga")
                {
                    format["vcodec"] = serde_json::json!("none");
                }
                if let Some(value) = json_i64(source, "width") {
                    format["width"] = serde_json::json!(value);
                }
                if let Some(value) = json_i64(source, "height")
                    .or_else(|| json_string(source, "label").and_then(jwplatform_label_height))
                {
                    format["height"] = serde_json::json!(value);
                }
                if let Some(value) = json_i64(source, "bitrate") {
                    format["tbr"] = serde_json::json!(value as f64 / 1000.0);
                }
                if let Some(value) = json_i64(source, "filesize") {
                    format["filesize"] = serde_json::json!(value);
                }
                formats.push(format);
            }
            if formats.is_empty() {
                continue;
            }
            let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(item_id));
            info.insert_if_some(
                "title",
                json_string(item, "title").map(unescape_html_attribute),
            );
            info.insert(
                "url",
                first.get("url").cloned().unwrap_or(serde_json::Value::Null),
            );
            info.insert(
                "ext",
                first
                    .get("ext")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!("mp4")),
            );
            info.insert("formats", serde_json::Value::Array(formats));
            info.insert_if_some(
                "description",
                json_string(item, "description").map(|value| html_text_fragment(value)),
            );
            info.insert_if_some(
                "thumbnail",
                json_string(item, "image")
                    .map(|value| resolve_url(&base_url, &proto_relative_url(value, "https:"))),
            );
            info.insert_if_some(
                "timestamp",
                json_i64(item, "pubdate").or_else(|| {
                    json_string(item, "pubdate")
                        .map(str::to_owned)
                        .and_then(parse_timestamp)
                }),
            );
            info.insert_if_some(
                "duration",
                json_f64(&response, "duration").or_else(|| json_f64(item, "duration")),
            );
            info.insert_if_some(
                "alt_title",
                json_string(item, "subtitle").map(|value| html_text_fragment(value)),
            );
            info.insert_if_some(
                "genre",
                json_string(item, "genre").map(|value| html_text_fragment(value)),
            );
            info.insert_if_some(
                "channel",
                json_string(item, "channel")
                    .or_else(|| json_string(item, "category"))
                    .map(str::to_owned),
            );
            info.insert_if_some("season_number", json_i64(item, "season"));
            info.insert_if_some("episode_number", json_i64(item, "episode"));
            info.insert_if_some("release_year", json_i64(item, "releasedate"));
            info.insert_if_some("age_limit", json_i64(item, "age_restriction"));
            if let Some(tracks) = item.get("tracks").and_then(serde_json::Value::as_array) {
                let mut subtitles = serde_json::Map::new();
                for track in tracks {
                    let Some(kind) = json_string(track, "kind") else {
                        continue;
                    };
                    if !matches!(kind.to_ascii_lowercase().as_str(), "captions" | "subtitles") {
                        continue;
                    }
                    let Some(raw_url) =
                        json_string(track, "file").filter(|value| !value.is_empty())
                    else {
                        continue;
                    };
                    let language = json_string(track, "label").unwrap_or("en");
                    let subtitle_url =
                        resolve_url(&base_url, &proto_relative_url(raw_url, "https:"));
                    subtitles
                        .entry(language.to_owned())
                        .or_insert_with(|| serde_json::json!([]))
                        .as_array_mut()
                        .expect("JWPlatform subtitle list")
                        .push(serde_json::json!({"url": subtitle_url}));
                }
                if !subtitles.is_empty() {
                    info.insert("subtitles", serde_json::Value::Object(subtitles));
                }
            }
            entries.push(info);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JWPlatform media {video_id} has no playable sources"),
            ));
        }
        if entries.len() == 1 {
            return Ok(ExtractorResult::single(
                entries.pop().expect("one JWPlatform entry"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&response, "title"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn jwplatform_label_height(label: &str) -> Option<i64> {
    let matcher = Regex::new(r#"(?i)\b(\d{3,4})p\b"#).ok()?;
    matcher
        .captures(label)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i64>().ok())
}

/// Native wrapper for Bundesliga pages that expose a JWPlatform media ID.
pub struct BundesligaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BundesligaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BundesligaExtractor {
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
        _context: &ExtractionContext,
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
                    "Bundesliga URL did not contain a media ID",
                )
            })?;
        Ok(ExtractorResult::Redirect {
            url: format!("jwplatform:{video_id}"),
            ie_key: Some("JWPlatform".to_owned()),
        })
    }
}

/// Native wrapper for OutsideTV pages that encode a JWPlatform media ID.
pub struct OutsideTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl OutsideTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for OutsideTvExtractor {
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
        _context: &ExtractionContext,
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
                    "OutsideTV URL did not contain a media ID",
                )
            })?;
        Ok(ExtractorResult::Redirect {
            url: format!("jwplatform:{video_id}"),
            ie_key: Some("JWPlatform".to_owned()),
        })
    }
}

/// Native wrapper for TeachingChannel pages that expose a JWPlatform media
/// ID in either the player data attribute or element ID.
pub struct TeachingChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl TeachingChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for TeachingChannelExtractor {
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
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "TeachingChannel URL did not contain a display ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let player_matcher = Regex::new(
            r#"(?is)(?:data-mid\s*=\s*["']|id\s*=\s*["']jw-video-player-)([a-zA-Z0-9]{8})"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid TeachingChannel player matcher: {error}"),
            )
        })?;
        let media_id = player_matcher
            .captures(&html)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("TeachingChannel page {display_id} has no JWPlatform media ID"),
                )
            })?;
        Ok(ExtractorResult::Redirect {
            url: format!("jwplatform:{media_id}"),
            ie_key: Some("JWPlatform".to_owned()),
        })
    }
}
