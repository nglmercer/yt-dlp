/// Native Gedi Digital page-embedded player-parameter extractor.
pub struct GediDigitalExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
    player_parameter_matcher: Regex,
}

impl GediDigitalExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let player_parameter_matcher = Regex::new(
            r#"(?x)PlayerFactory\.setParam\(\s*['\"](?P<type>format|param)['\"]\s*,\s*['\"](?P<name>[^'\"]+)['\"]\s*,\s*['\"](?P<value>[^'\"]*)['\"]\s*\)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Gedi Digital player parameter matcher: {error}"),
            )
        })?;
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
            player_parameter_matcher,
        })
    }
}

impl InfoExtractor for GediDigitalExtractor {
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
                "Gedi Digital URL did not match its native pattern",
            )
        })?;
        let page_url = captures
            .name("base_url")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| url.to_owned());
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Gedi Digital URL has no ID")
            })?;
        let response = context.get(&page_url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let title = html_meta_value(&webpage, "twitter:title")
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .map(|value| unescape_html_attribute(&value))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Gedi Digital video {video_id} has no title"),
                )
            })?;
        let mut formats = Vec::new();
        let mut thumbnail = None;
        let mut duration = None;
        for captures in self.player_parameter_matcher.captures_iter(&webpage).flatten() {
            let Some(parameter_type) = captures.name("type").map(|value| value.as_str()) else {
                continue;
            };
            let Some(parameter_name) = captures.name("name").map(|value| value.as_str()) else {
                continue;
            };
            let Some(parameter_value) = captures.name("value").map(|value| value.as_str()) else {
                continue;
            };
            let parameter_value = unescape_html_attribute(parameter_value);
            if parameter_type == "param" {
                match parameter_name {
                    "image_full" | "image" if thumbnail.is_none() => {
                        thumbnail = Some(parameter_value);
                    }
                    "videoDuration" => {
                        duration = parameter_value.parse::<i64>().ok();
                    }
                    _ => {}
                }
                continue;
            }
            if [
                "video-hds-vod-ec",
                "video-hls-vod-ec",
                "video-viralize",
                "video-youtube-pfp",
            ]
            .contains(&parameter_name)
            {
                continue;
            }
            if !parameter_value.starts_with("http://") && !parameter_value.starts_with("https://")
            {
                continue;
            }
            let extension = yt_dlp_core::determine_ext(Some(&parameter_value), "mp4");
            let mut format = serde_json::json!({
                "url": parameter_value,
                "format_id": parameter_name,
                "ext": extension,
            });
            if extension == "m3u8" {
                format["protocol"] = serde_json::json!("m3u8_native");
                format["ext"] = serde_json::json!("mp4");
            } else if extension == "mp3" {
                if let Some(abr) = gedidigital_audio_bitrate(parameter_name, format["url"].as_str())
                {
                    format["abr"] = serde_json::json!(abr);
                    format["tbr"] = serde_json::json!(abr);
                }
                format["acodec"] = serde_json::json!("mp3");
                format["vcodec"] = serde_json::json!("none");
            } else {
                if let Some((height, vbr)) = gedidigital_video_quality(parameter_name) {
                    format["height"] = serde_json::json!(height);
                    if let Some(vbr) = vbr {
                        format["vbr"] = serde_json::json!(vbr);
                    }
                } else if let Some(vbr) = gedidigital_video_bitrate(&parameter_value) {
                    format["vbr"] = serde_json::json!(vbr);
                }
            }
            if !formats.iter().any(|existing: &serde_json::Value| {
                existing.get("url") == format.get("url")
            }) {
                formats.push(format);
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Gedi Digital video {video_id} has no playable formats"),
            ));
        }
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Gedi Digital video {video_id} has no first format URL"),
                )
            })?;
        let first_ext = formats
            .first()
            .and_then(|format| format.get("ext"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "description",
            html_meta_value(&webpage, "twitter:description")
                .or_else(|| html_meta_value(&webpage, "og:description"))
                .or_else(|| html_meta_value(&webpage, "description"))
                .map(|value| unescape_html_attribute(&value)),
        );
        info.insert_if_some(
            "thumbnail",
            thumbnail.or_else(|| html_meta_value(&webpage, "og:image")),
        );
        info.insert_if_some("duration", duration);
        info.insert("url", serde_json::json!(first_url));
        info.insert("ext", serde_json::json!(first_ext));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn gedidigital_audio_bitrate(format_id: &str, media_url: Option<&str>) -> Option<i64> {
    let matcher = Regex::new(r#"-mp3-audio-(\d+)"#).ok()?;
    matcher
        .captures(format_id)
        .ok()
        .flatten()
        .or_else(|| matcher.captures(media_url?).ok().flatten())
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i64>().ok())
}

fn gedidigital_video_quality(format_id: &str) -> Option<(i64, Option<i64>)> {
    let matcher = Regex::new(r#"^video-rrtv-(\d+)(?:-(\d+))?$"#).ok()?;
    let captures = matcher.captures(format_id).ok().flatten()?;
    let height = captures.get(1)?.as_str().parse::<i64>().ok()?;
    let vbr = captures
        .get(2)
        .and_then(|value| value.as_str().parse::<i64>().ok());
    Some((height, vbr))
}

fn gedidigital_video_bitrate(media_url: &str) -> Option<i64> {
    let matcher = Regex::new(r#"-video-rrtv-(\d+)"#).ok()?;
    matcher
        .captures(media_url)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i64>().ok())
}
