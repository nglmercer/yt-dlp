/// Native ERTFLIX codename/API media extractor.
pub struct ErtflixCodenameExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ErtflixCodenameExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ErtflixCodenameExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "ERTFLIX URL has no codename")
            })?;
        let media_info = ertflix_api_request(
            context,
            "Player/AcquireContent",
            1,
            &[("codename".to_owned(), video_id.clone())],
        )?;
        let formats = ertflix_main_formats(&media_info);
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ERTFLIX content {video_id} has no playable main media"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                url.strip_prefix("ertflix:")
                    .filter(|value| !value.is_empty())
                    .unwrap_or("ERTFLIX content")
            ),
        );
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

fn ertflix_api_request(
    context: &ExtractionContext,
    method: &str,
    api_version: u8,
    params: &[(String, String)],
) -> Result<serde_json::Value, ExtractorError> {
    let endpoint = format!("https://api.app.ertflix.gr/v{api_version}/{method}");
    let mut request = Request::new(&endpoint);
    let headers = serde_json::json!({
        "X-Api-Date-Format": "iso",
        "X-Api-Camel-Case": false,
    })
    .to_string();
    let mut query = vec![
        ("platformCodename".to_owned(), "www".to_owned()),
        ("$headers".to_owned(), headers),
    ];
    query.extend(params.iter().cloned());
    request.update_query(&query);
    let response = context.request(&request)?;
    ertflix_decode_api_response(response.body(), response.url())
}

fn ertflix_main_formats(media_info: &serde_json::Value) -> Vec<serde_json::Value> {
    let Some(media_files) = media_info
        .get("MediaFiles")
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    let mut formats = Vec::new();
    for media in media_files {
        if json_string(media, "RoleCodename") != Some("main") {
            continue;
        }
        let Some(media_formats) = media
            .get("Formats")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for source in media_formats {
            let Some(media_url) = json_string(source, "Url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
            let is_hls = extension.eq_ignore_ascii_case("m3u8");
            let is_dash = extension.eq_ignore_ascii_case("mpd");
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": if is_hls {
                    "hls".to_owned()
                } else if is_dash {
                    "dash".to_owned()
                } else {
                    json_value_string(source.get("Id")).unwrap_or_else(|| "http".to_owned())
                },
                "protocol": if is_hls {
                    "m3u8_native"
                } else if is_dash {
                    "http_dash_segments"
                } else {
                    "http"
                },
                "ext": if is_hls || is_dash {
                    "mp4"
                } else {
                    extension.as_str()
                },
            });
            if is_hls || is_dash {
                format["vcodec"] = serde_json::json!("unknown");
            }
            formats.push(format);
        }
    }
    formats
}
