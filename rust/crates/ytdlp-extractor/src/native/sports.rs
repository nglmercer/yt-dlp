/// Native Academic Earth course playlist extractor.
pub struct AcademicEarthCourseExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AcademicEarthCourseExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AcademicEarthCourseExtractor {
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
        let playlist_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Academic Earth playlist URL has no ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(
            r#"(?is)<h1\b[^>]*\bclass\s*=\s*["'][^"']*playlist-name[^"']*["'][^>]*>(.*?)</h1>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Academic Earth playlist {playlist_id} has no title"),
            )
        })?;
        let description =
            Regex::new(r#"(?is)<p\b[^>]*\bclass\s*=\s*["'][^"']*excerpt[^"']*["'][^>]*>(.*?)</p>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
        let link_matcher = Regex::new(
            r#"(?is)<li\b[^>]*\bclass\s*=\s*["'][^"']*lecture-preview[^"']*["'][^>]*>\s*<a\b[^>]*\btarget\s*=\s*["']_blank["'][^>]*\bhref\s*=\s*["']([^"']+)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Academic Earth lecture matcher: {error}"),
            )
        })?;
        let base_url = url::Url::parse(url).ok();
        let mut entries = Vec::new();
        for captures in link_matcher.captures_iter(&html).flatten() {
            let Some(raw_url) = captures.get(1).map(|value| value.as_str().trim()) else {
                continue;
            };
            let entry_url = base_url
                .as_ref()
                .and_then(|base| base.join(raw_url).ok())
                .map_or_else(
                    || proto_relative_url(raw_url, "https:"),
                    |value| value.to_string(),
                );
            entries.push(native_url_result(&entry_url));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Premiership Rugby article/JWPlatform HLS extractor.
pub struct PremiershipRugbyExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PremiershipRugbyExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PremiershipRugbyExtractor {
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
                    "Premiership Rugby URL has no article slug",
                )
            })?;
        let data_url = format!(
            "https://article-cms-api.incrowdsports.com/v2/articles/slug/{display_id}?clientId=PRL"
        );
        let response = context.get_json(&data_url)?;
        let article = response
            .get("data")
            .and_then(|data| data.get("article"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Premiership Rugby article {display_id} has no article object"),
                )
            })?;
        let hero = article.get("heroMedia").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Premiership Rugby article {display_id} has no hero media"),
            )
        })?;
        let content = hero.get("content").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Premiership Rugby article {display_id} has no media content"),
            )
        })?;
        let media_url = json_string(content, "videoLink").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Premiership Rugby article {display_id} has no video link"),
            )
        })?;
        let video_id = json_string(content, "sourceSystemId").unwrap_or(&display_id);
        let duration = content
            .get("metadata")
            .and_then(|metadata| json_f64(metadata, "msDuration"))
            .map(|milliseconds| milliseconds / 1000.0);
        let categories = article
            .get("categories")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                serde_json::Value::Array(
                    items
                        .iter()
                        .filter_map(|item| json_string(item, "text").map(str::to_owned))
                        .map(serde_json::Value::String)
                        .collect(),
                )
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(hero, "title"));
        info.insert_if_some("thumbnail", json_string(content, "videoThumbnail"));
        info.insert_if_some("duration", duration);
        info.insert_if_some("tags", article.get("tags").cloned());
        info.insert_if_some("categories", categories);
        info.insert_if_some(
            "subtitles",
            content
                .get("subtitles")
                .cloned()
                .or_else(|| content.get("captions").cloned()),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native MatchiTV Next.js/HLS extractor.
pub struct MatchiTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MatchiTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MatchiTvExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "MatchiTV URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let loaded_media = html_script_json(&html, "__NEXT_DATA__")
            .ok()
            .and_then(|data| data.get("props").cloned())
            .and_then(|props| props.get("pageProps").cloned())
            .and_then(|page_props| page_props.get("loadedMedia").cloned())
            .unwrap_or(serde_json::Value::Null);
        let court = json_string(&loaded_media, "courtDescription");
        let start = json_string(&loaded_media, "startDateTime");
        let title = match (court, start) {
            (Some(court), Some(start)) => format!("{court} {start}"),
            (Some(court), None) => court.to_owned(),
            (None, Some(start)) => start.to_owned(),
            (None, None) => video_id.clone(),
        };
        let media_url = format!(
            "https://streams.padelgo.tv/v2/streams/m3u8/{video_id}/anonymous/playlist.m3u8"
        );
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "thumbnail",
            serde_json::json!(format!("https://thumbnails.padelgo.tv/{video_id}.jpg")),
        );
        info.insert_if_some("upload_date", start.and_then(date_digits));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native SZTV.hu VOD extractor.
pub struct SztvHuExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SztvHuExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SztvHuExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "SZTV URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let video_file = Regex::new(r#"(?is)\bfile\s*:\s*["'][^"']*?:([^"']+)["']\s*,"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().trim().to_owned())
            .map(|value| {
                value
                    .rsplit_once(':')
                    .filter(|(_, suffix)| !suffix.contains('/'))
                    .map_or(value.clone(), |(_, suffix)| suffix.to_owned())
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("SZTV video {video_id} has no media file"),
                )
            })?;
        let title = html_meta_value(&html, "title")
            .map(|value| {
                value
                    .split(" - ")
                    .next()
                    .unwrap_or(&value)
                    .trim()
                    .to_owned()
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| video_id.clone());
        let media_url = format!(
            "http://media.sztv.hu/vod/{}",
            video_file.trim_start_matches('/')
        );
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", html_meta_value(&html, "description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
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

/// Native Arnes Video public-media API extractor.
pub struct ArnesExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ArnesExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ArnesExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Arnes URL has no ID")
            })?;
        let response = context.get_json(&format!(
            "https://video.arnes.si/api/public/video/{video_id}"
        ))?;
        let video = response.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Arnes video {video_id} has no data object"),
            )
        })?;
        let title = json_string(video, "title").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Arnes video {video_id} has no title"),
            )
        })?;
        let media = video
            .get("media")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Arnes video {video_id} has no media records"),
                )
            })?;
        let mut formats = Vec::new();
        for item in media {
            let Some(raw_url) = json_string(item, "url") else {
                continue;
            };
            let media_url = resolve_url("https://video.arnes.si", raw_url);
            let format_id = json_string(item, "format")
                .and_then(|value| value.strip_prefix("FORMAT_"))
                .map(str::to_owned);
            let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "format_note": json_string(item, "formatTranslation"),
                "width": json_i64(item, "width"),
                "height": json_i64(item, "height"),
                "ext": ext,
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Arnes video {video_id} has no playable media"),
            )
        })?;
        let channel = video.get("channel").unwrap_or(&serde_json::Value::Null);
        let channel_id = json_string(channel, "url");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "thumbnail",
            json_string(video, "thumbnailUrl")
                .map(|value| resolve_url("https://video.arnes.si", value)),
        );
        info.insert_if_some("description", json_string(video, "description"));
        info.insert_if_some("license", json_string(video, "license"));
        info.insert_if_some("creator", json_string(video, "author"));
        info.insert_if_some(
            "timestamp",
            json_string(video, "creationTime")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("channel", json_string(channel, "name"));
        info.insert_if_some("channel_id", channel_id);
        info.insert_if_some(
            "channel_url",
            channel_id.map(|value| format!("https://video.arnes.si/?channel={value}")),
        );
        info.insert_if_some(
            "duration",
            json_f64(video, "duration").map(|milliseconds| milliseconds / 1000.0),
        );
        info.insert_if_some("view_count", json_i64(video, "views"));
        info.insert_if_some("tags", video.get("hashtags").cloned());
        info.insert_if_some(
            "start_time",
            url_query_value(url, "t").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native CJSW episode audio-page extractor.
pub struct CjswExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CjswExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CjswExtractor {
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
                "CJSW URL did not match its native pattern",
            )
        })?;
        let program = captures
            .name("program")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CJSW URL has no program")
            })?;
        let episode_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CJSW URL has no episode ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(
            r#"(?is)<h1\b[^>]*\bclass\s*=\s*["'][^"']*episode-header__title[^"']*["'][^>]*>([^<]+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str().trim()))
        .or_else(|| {
            Regex::new(r#"(?is)\bdata-audio-title\s*=\s*["']([^"']+)["']"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| unescape_html_attribute(value.as_str().trim()))
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CJSW episode {episode_id} has no title"),
            )
        })?;
        let audio_url = Regex::new(r#"(?is)<button\b[^>]*\bdata-audio-src\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("CJSW episode {episode_id} has no audio URL"),
                )
            })?;
        let audio_id =
            Regex::new(r#"(?i)/([\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12})\.mp3"#)
                .ok()
                .and_then(|matcher| matcher.captures(&audio_url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
                .unwrap_or_else(|| format!("{program}/{episode_id}"));
        let ext = yt_dlp_core::determine_ext(Some(&audio_url), "mp3");
        let description = Regex::new(r#"(?is)<p\b[^>]*>(.*?)</p>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let series = Regex::new(r#"(?is)\bdata-showname\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .unwrap_or(program);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert("series", serde_json::json!(series));
        info.insert("episode_id", serde_json::json!(episode_id));
        info.insert("url", serde_json::json!(audio_url.clone()));
        info.insert("ext", serde_json::json!(ext));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": audio_url,
                "format_id": "source",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Daystar Lightcast configuration/HLS extractor.
pub struct DaystarClipExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DaystarClipExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DaystarClipExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Daystar URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let iframe_url = Regex::new(r#"(?is)<iframe\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daystar clip {video_id} has no iframe"),
                )
            })?;
        let config_url = iframe_url.replace("player.php", "config2.php");
        let config_response = context.get(&config_url)?;
        let config_html = String::from_utf8_lossy(config_response.body());
        let sources = json_array_after_marker(&config_html, "sources")
            .and_then(|value| value.as_array().cloned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daystar clip {video_id} has no source list"),
                )
            })?;
        let mut formats = Vec::new();
        for source in sources {
            let Some(raw_url) = json_string(&source, "file") else {
                continue;
            };
            if json_string(&source, "type").map(|value| value.eq_ignore_ascii_case("m3u8"))
                != Some(true)
            {
                continue;
            }
            let media_url = resolve_url("https://www.lightcast.com/embed/", raw_url);
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Daystar clip {video_id} has no HLS source"),
            )
        })?;
        let thumbnail = Regex::new(r#"(?is)\bimage\s*:\s*["']([^"']+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&config_html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(&config_url, value.as_str()));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_meta_value(&html, "twitter:title")),
        );
        info.insert_if_some(
            "description",
            html_meta_value(&html, "og:description")
                .or_else(|| html_meta_value(&html, "twitter:description")),
        );
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native DCTP versioned REST/API extractor.
pub struct DctpTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DctpTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DctpTvExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DCTP URL has no slug")
            })?;
        let base_url = "http://dctp-ivms2-restapi.s3.amazonaws.com";
        let version = context.get_json(&format!("{base_url}/version.json"))?;
        let version_name = json_string(&version, "version_name").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "DCTP version response has no version_name",
            )
        })?;
        let restapi_base = format!("{base_url}/{version_name}/restapi");
        let info = context.get_json(&format!("{restapi_base}/slugs/{display_id}.json"))?;
        let object_id = json_value_string(info.get("object_id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DCTP film {display_id} has no object ID"),
            )
        })?;
        let media = context.get_json(&format!("{restapi_base}/media/{object_id}.json"))?;
        let uuid = json_string(&media, "uuid").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DCTP film {display_id} has no media UUID"),
            )
        })?;
        let title = json_string(&media, "title").unwrap_or(&display_id);
        let is_wide = json_bool(&media, "is_wide").unwrap_or(false);
        let mut formats = Vec::new();
        let mut add_formats = |suffix: &str| {
            let filename = format!("{uuid}_dctp_{suffix}.m4v");
            formats.push(serde_json::json!({
                "format_id": format!("hls-{suffix}"),
                "url": format!("https://cdn-segments.dctp.tv/{filename}/playlist.m3u8"),
                "protocol": "m3u8_native",
                "ext": "m4v",
            }));
            formats.push(serde_json::json!({
                "format_id": format!("s3-{suffix}"),
                "url": format!("https://completed-media.s3.amazonaws.com/{filename}"),
                "ext": "m4v",
            }));
            formats.push(serde_json::json!({
                "format_id": format!("http-{suffix}"),
                "url": format!("https://cdn-media.dctp.tv/{filename}"),
                "ext": "m4v",
            }));
        };
        add_formats(&format!("0500_{}", if is_wide { "16x9" } else { "4x3" }));
        if is_wide {
            add_formats("720p");
        }
        let thumbnails = media
            .get("images")
            .and_then(serde_json::Value::as_array)
            .map(|images| {
                serde_json::Value::Array(
                    images
                        .iter()
                        .filter_map(|image| {
                            let image_url = json_string(image, "url")?;
                            let mut thumbnail = serde_json::Map::new();
                            thumbnail.insert(
                                "url".to_owned(),
                                serde_json::Value::String(image_url.to_owned()),
                            );
                            if let Some(width) = json_i64(image, "width") {
                                thumbnail.insert("width".to_owned(), serde_json::json!(width));
                            }
                            if let Some(height) = json_i64(image, "height") {
                                thumbnail.insert("height".to_owned(), serde_json::json!(height));
                            }
                            Some(serde_json::Value::Object(thumbnail))
                        })
                        .collect(),
                )
            })
            .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()));
        let first = formats.first().cloned().expect("DCTP format");
        let mut result = InfoDict::new();
        result.insert("id", serde_json::json!(uuid));
        result.insert("display_id", serde_json::json!(display_id));
        result.insert("title", serde_json::json!(title));
        result.insert_if_some("alt_title", json_string(&media, "subtitle"));
        result.insert_if_some(
            "description",
            json_string(&media, "description").or_else(|| json_string(&media, "teaser")),
        );
        result.insert_if_some(
            "timestamp",
            json_string(&media, "created")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        result.insert_if_some(
            "duration",
            json_f64(&media, "duration_in_ms").map(|milliseconds| milliseconds / 1000.0),
        );
        result.insert_if_some("thumbnails", thumbnails);
        result.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        result.insert("ext", serde_json::json!("m4v"));
        result.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(result))
    }
}
