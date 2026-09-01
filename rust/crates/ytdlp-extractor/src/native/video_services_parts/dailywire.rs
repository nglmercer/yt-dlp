/// Native Daily Wire Next.js episode/video and podcast extractors.
pub struct DailyWireExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DailyWireExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DailyWireExtractor {
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
                "Daily Wire URL did not match its native pattern",
            )
        })?;
        let display_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Daily Wire URL has no display ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let next_data = html_script_json(&webpage, "__NEXT_DATA__")?;
        let page_props = next_data
            .get("props")
            .and_then(|props| props.get("pageProps"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daily Wire page {display_id} has no Next.js page props"),
                )
            })?;
        if self.descriptor.key == "DailyWirePodcastIE" {
            let episode = page_props.get("episode").ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daily Wire podcast {display_id} has no episode data"),
                )
            })?;
            let audio_id = json_string(episode, "audioMuxPlaybackId")
                .filter(|value| !value.is_empty())
                .unwrap_or("VUsAipTrBVSgzw73SpC2DAJD401TYYwEp");
            let media_url = format!("https://stream.media.dailywire.com/{audio_id}/audio.m4a");
            let mut info = InfoDict::new();
            info.insert(
                "id",
                episode
                    .get("id")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!(display_id)),
            );
            info.insert("url", serde_json::json!(media_url));
            info.insert("ext", serde_json::json!("m4a"));
            info.insert("display_id", serde_json::json!(display_id));
            info.insert_if_some("title", json_string(episode, "title"));
            info.insert_if_some("duration", json_f64(episode, "duration"));
            info.insert_if_some("thumbnail", json_string(episode, "thumbnail"));
            info.insert_if_some("description", json_string(episode, "description"));
            info.insert(
                "formats",
                serde_json::json!([{
                    "url": media_url,
                    "format_id": "http",
                    "protocol": "http",
                    "ext": "m4a",
                    "vcodec": "none",
                }]),
            );
            info.insert("subtitles", serde_json::json!({}));
            return Ok(ExtractorResult::single(info));
        }

        let episode = if captures.name("sites_type").map(|value| value.as_str()) == Some("videos") {
            page_props
                .get("videoData")
                .and_then(|data| data.get("video"))
        } else {
            page_props
                .get("episodeData")
                .and_then(|data| data.get("episode"))
        }
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Daily Wire page {display_id} has no video/episode data"),
            )
        })?;
        let mut media_urls = Vec::new();
        if let Some(segments) = episode.get("segments").and_then(serde_json::Value::as_array) {
            for segment in segments {
                for key in ["videoUrl", "video", "audio"] {
                    if let Some(media_url) = json_string(segment, key)
                        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
                    {
                        if !media_urls.contains(&media_url.to_owned()) {
                            media_urls.push(media_url.to_owned());
                        }
                    }
                }
            }
        }
        if let Some(media_url) = json_string(episode, "videoUrl")
            .or_else(|| json_string(episode, "audio"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            if !media_urls.contains(&media_url.to_owned()) {
                media_urls.push(media_url.to_owned());
            }
        }
        if media_urls.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Daily Wire page {display_id} has no media URLs"),
            ));
        }
        let formats = media_urls
            .iter()
            .enumerate()
            .map(|(index, media_url)| {
                let detected_ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
                if detected_ext.eq_ignore_ascii_case("m3u8") {
                    serde_json::json!({
                        "url": media_url,
                        "format_id": if index == 0 { "hls" } else { "hls-{index}" },
                        "protocol": "m3u8_native",
                        "ext": "mp4",
                    })
                } else {
                    serde_json::json!({
                        "url": media_url,
                        "format_id": if index == 0 { "http".to_owned() } else { format!("http-{index}") },
                        "protocol": "http",
                        "ext": detected_ext,
                    })
                }
            })
            .collect::<Vec<_>>();
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let title = json_string(episode, "title")
            .or_else(|| json_string(episode, "name"))
            .filter(|value| !value.is_empty())
            .unwrap_or(&display_id);
        let creator = episode
            .get("createdBy")
            .map(|created_by| {
                ["firstName", "lastName"]
                    .into_iter()
                    .filter_map(|key| json_string(created_by, key).filter(|value| !value.is_empty()))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|value| !value.is_empty());
        let mut info = InfoDict::new();
        info.insert(
            "id",
            episode
                .get("id")
                .cloned()
                .unwrap_or_else(|| serde_json::json!(display_id)),
        );
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", json_string(episode, "description"));
        info.insert_if_some("creator", creator);
        info.insert_if_some("duration", json_f64(episode, "duration"));
        info.insert_if_some("is_live", json_bool(episode, "isLive"));
        info.insert_if_some(
            "thumbnail",
            json_string(episode, "thumbnail").or_else(|| json_string(episode, "image")),
        );
        if let Some(show) = episode.get("show") {
            info.insert_if_some("series_id", json_string(show, "id"));
            info.insert_if_some("series", json_string(show, "name"));
        }
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
