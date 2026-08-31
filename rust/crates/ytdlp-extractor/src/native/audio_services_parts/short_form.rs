/// Native Coub API extractor. All media variants and counters are read from
/// the Coub JSON response and represented as ordinary Rust format records.
pub struct CoubExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CoubExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CoubExtractor {
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
                "Coub URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Coub URL has no ID")
            })?;
        let coub = context.get_json(&format!("http://coub.com/api/v2/coubs/{video_id}.json"))?;
        if let Some(error) = json_string(&coub, "error") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("Coub API error: {error}"),
            ));
        }
        let file_versions = coub
            .get("file_versions")
            .and_then(serde_json::Value::as_object);
        let mut formats = Vec::new();
        if let Some(html5) = file_versions
            .and_then(|versions| versions.get("html5"))
            .and_then(serde_json::Value::as_object)
        {
            for (kind, media_type) in [("video", "video"), ("audio", "audio")] {
                let Some(qualities) = html5.get(kind).and_then(serde_json::Value::as_object) else {
                    continue;
                };
                for (quality, item) in qualities {
                    let Some(media_url) = item.get("url").and_then(serde_json::Value::as_str)
                    else {
                        continue;
                    };
                    let default_ext = if media_type == "audio" { "mp3" } else { "mp4" };
                    let ext = yt_dlp_core::determine_ext(Some(media_url), default_ext);
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": format!("html5-{media_type}-{quality}"),
                        "ext": ext,
                        "quality": match quality.as_str() {
                            "low" => 0,
                            "med" => 1,
                            "high" => 2,
                            "higher" => 3,
                            _ => -1,
                        },
                        "vcodec": if media_type == "audio" { "none" } else { "unknown" },
                        "acodec": if media_type == "video" { "none" } else { "unknown" },
                    });
                    if let Some(size) = json_i64(item, "size") {
                        format["filesize"] = serde_json::json!(size);
                    }
                    formats.push(format);
                }
            }
        }
        if let Some(item) = file_versions
            .and_then(|versions| versions.get("iphone"))
            .and_then(serde_json::Value::as_object)
        {
            if let Some(media_url) = json_string(&serde_json::Value::Object(item.clone()), "url") {
                formats.push(serde_json::json!({
                    "url": media_url,
                    "format_id": "iphone",
                    "ext": yt_dlp_core::determine_ext(Some(media_url), "mp4"),
                }));
            }
        }
        if let Some(media_url) = file_versions
            .and_then(|versions| versions.get("mobile"))
            .and_then(|mobile| json_string(mobile, "audio_url"))
        {
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "mobile-audio",
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mp3"),
                "vcodec": "none",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Coub API returned no playable formats for {video_id}"),
            ));
        }
        let channel = coub.get("channel");
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&coub, "title"));
        info.insert_if_some("thumbnail", json_string(&coub, "picture"));
        info.insert_if_some("duration", json_f64(&coub, "duration"));
        info.insert_if_some(
            "timestamp",
            json_string(&coub, "published_at")
                .or_else(|| json_string(&coub, "created_at"))
                .and_then(yt_dlp_core::parse_iso8601),
        );
        info.insert_if_some(
            "uploader",
            channel.and_then(|value| json_string(value, "title")),
        );
        info.insert_if_some(
            "uploader_id",
            channel.and_then(|value| json_string(value, "permalink")),
        );
        info.insert_if_some(
            "view_count",
            json_i64(&coub, "views_count").or_else(|| json_i64(&coub, "views_increase_count")),
        );
        info.insert_if_some("like_count", json_i64(&coub, "likes_count"));
        info.insert_if_some("repost_count", json_i64(&coub, "recoubs_count"));
        if let Some(age_restricted) = json_bool(&coub, "age_restricted")
            .or_else(|| json_bool(&coub, "age_restricted_by_admin"))
        {
            info.insert(
                "age_limit",
                serde_json::json!(if age_restricted { 18 } else { 0 }),
            );
        }
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
        Ok(ExtractorResult::single(info))
    }
}

/// Native Vocaroo direct-audio extractor. The media host is selected from the
/// ID shape and a Rust HEAD request preserves the upload timestamp header.
pub struct VocarooExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl VocarooExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for VocarooExtractor {
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
                "Vocaroo URL did not match its native pattern",
            )
        })?;
        let audio_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Vocaroo URL has no ID")
            })?;
        let media_subdomain =
            if audio_id.len() == 10 || (audio_id.len() == 12 && audio_id.starts_with('1')) {
                "media1"
            } else {
                "media"
            };
        let media_url = format!("https://{media_subdomain}.vocaroo.com/mp3/{audio_id}");
        let mut request = Request::new(&media_url);
        request.set_method("HEAD").map_err(map_request_error)?;
        request.headers_mut().set("Referer", "https://vocaroo.com/");
        let response = context.request(&request)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(""));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert(
            "http_headers",
            serde_json::json!({"Referer": "https://vocaroo.com/"}),
        );
        if let Some(timestamp) = response
            .headers()
            .get("x-bz-upload-timestamp")
            .and_then(|value| value.parse::<f64>().ok())
            .map(|value| value / 1000.0)
        {
            info.insert("timestamp", serde_json::json!(timestamp));
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Freesound HTML/Open Graph extractor. The page metadata is enough to
/// build the same low/high audio format set without browser execution.
pub struct FreesoundExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FreesoundExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FreesoundExtractor {
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
                "Freesound URL did not match its native pattern",
            )
        })?;
        let audio_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Freesound URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let audio_url = html_meta_value(&html, "og:audio").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Freesound page {audio_id} has no audio URL"),
            )
        })?;
        let audio_url = audio_url
            .strip_prefix("https://freesound.org")
            .filter(|value| value.starts_with("http"))
            .unwrap_or(&audio_url)
            .to_owned();
        let mut audio_urls = vec![audio_url.clone()];
        if audio_url.contains("-lq.mp3") {
            audio_urls.push(audio_url.replace("-lq.mp3", "-hq.mp3"));
        }
        let channels = Regex::new(r#"(?is)Channels\s*</dt>\s*<dd[^>]*>(.*?)</dd>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| {
                captures
                    .get(1)
                    .map(|value| html_text_fragment(value.as_str()))
            });
        let formats = audio_urls
            .into_iter()
            .enumerate()
            .map(|(quality, media_url)| {
                serde_json::json!({
                    "url": media_url,
                    "format_id": if quality == 0 { "lq" } else { "hq" },
                    "ext": "mp3",
                    "format_note": channels.as_deref(),
                    "quality": quality,
                    "vcodec": "none",
                })
            })
            .collect::<Vec<_>>();
        let duration =
            Regex::new(r#"(?is)class\s*=\s*["'][^"']*\bduration\b[^"']*["'][^>]*>([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| value.as_str().trim().to_owned())
                })
                .and_then(|value| {
                    value
                        .parse::<f64>()
                        .map(|value| value / 1000.0)
                        .ok()
                        .or_else(|| yt_dlp_core::parse_duration(&value))
                });
        let description =
            Regex::new(r#"(?is)\bid\s*=\s*["']sound_description["'][^>]*>(.*?)</div>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                });
        let tags = Regex::new(r#"(?is)<a\b[^>]*>([^<]+)</a>"#)
            .ok()
            .and_then(|matcher| {
                let container = Regex::new(
                    r#"(?is)class\s*=\s*["'][^"']*\btags\b[^"']*["'][^>]*>(.*?)</(?:div|section)>"#,
                )
                .ok()?;
                let captures = container.captures(&html).ok().flatten()?;
                let body = captures.get(1)?.as_str();
                let values = matcher
                    .captures_iter(body)
                    .flatten()
                    .filter_map(|captures| {
                        captures
                            .get(1)
                            .map(|value| html_text_fragment(value.as_str()))
                    })
                    .filter(|tag| !tag.is_empty())
                    .map(serde_json::Value::String)
                    .collect::<Vec<_>>();
                (!values.is_empty()).then_some(serde_json::Value::Array(values))
            });
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:audio:title").or_else(|| html_meta_value(&html, "og:title")),
        );
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("uploader", html_meta_value(&html, "og:audio:artist"));
        info.insert_if_some("tags", tags);
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}
