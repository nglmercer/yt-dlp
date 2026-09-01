/// Native Hungama video, song, and album/playlist extractors.
pub struct HungamaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HungamaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HungamaExtractor {
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
        let video_id = hunga_match_id(&self.matcher, url, "Hungama video")?;
        let video_json = hunga_video_url(context, &video_id)?;
        let stream_url = json_string(&video_json, "stream_url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Hungama video {video_id} has no stream URL"),
                )
            })?;
        let format = hunga_hls_format(stream_url, &video_id)?;
        let metadata = hunga_call_api(context, "movie", &video_id)?;
        let head = metadata
            .get("head")
            .and_then(|head| head.get("data"))
            .unwrap_or(&serde_json::Value::Null);
        let misc = head.get("misc").unwrap_or(&serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(head, "title"));
        info.insert_if_some("description", json_string(misc, "description"));
        info.insert_if_some("duration", json_i64(head, "duration"));
        info.insert_if_some(
            "timestamp",
            json_string(head, "releasedate")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("view_count", json_i64(misc, "playcount"));
        info.insert_if_some("thumbnail", json_string(head, "image"));
        info.insert_if_some("tags", hunga_tags(misc.get("keywords")));
        if let Some(subtitle_url) = json_string(&video_json, "sub_title")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            info.insert(
                "subtitles",
                serde_json::json!({"en": [{"url": subtitle_url, "ext": "vtt"}]}),
            );
        } else {
            info.insert("subtitles", serde_json::json!({}));
        }
        info.insert("url", serde_json::json!(stream_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::json!([format]));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Hungama audio track extractor.
pub struct HungamaSongExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HungamaSongExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HungamaSongExtractor {
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
        let audio_id = hunga_match_id(&self.matcher, url, "Hungama song")?;
        let mut track_request =
            Request::new(&format!("https://www.hungama.com/audio-player-data/track/{audio_id}"));
        track_request.update_query(&[("_country".to_owned(), "IN".to_owned())]);
        let track_response = context.request(&track_request)?;
        let track_data = serde_json::from_slice::<serde_json::Value>(track_response.body())
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Hungama song JSON for {audio_id}: {error}"),
                )
            })?;
        let track = track_data
            .as_array()
            .and_then(|tracks| tracks.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Hungama song {audio_id} has no track data"),
                )
            })?;
        let track_name = json_string(track, "song_name").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Hungama song {audio_id} has no title"),
            )
        })?;
        let media_source = json_string(track, "file")
            .or_else(|| json_string(track, "preview_link"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Hungama song {audio_id} has no media source"),
                )
            })?;
        let media_json = context.get_json(media_source)?;
        let media = media_json.get("response").unwrap_or(&media_json);
        let media_url = json_string(media, "media_url")
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Hungama song {audio_id} has no media URL"),
                )
            })?;
        let extension = json_string(media, "type")
            .map(str::to_owned)
            .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(media_url), "mp3"));
        let artist = json_string(track, "singer_name");
        let title = artist
            .map(|artist| format!("{artist} - {track_name}"))
            .unwrap_or_else(|| track_name.to_owned());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(title));
        info.insert("track", serde_json::json!(track_name));
        info.insert_if_some("artist", artist);
        info.insert_if_some("album", json_string(track, "album_name"));
        info.insert_if_some(
            "release_year",
            json_string(track, "date").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(track, "img_src").or_else(|| json_string(track, "album_image")),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": extension,
                "vcodec": "none",
            }]),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Hungama album and playlist extractor.
pub struct HungamaAlbumPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HungamaAlbumPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HungamaAlbumPlaylistExtractor {
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
                "Hungama album URL did not match its native pattern",
            )
        })?;
        let path = captures
            .name("path")
            .map(|value| value.as_str().trim_end_matches('s').to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Hungama album has no path")
            })?;
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Hungama album has no ID")
            })?;
        let data = hunga_call_api(context, &path, &playlist_id)?;
        let rows = data
            .get("body")
            .and_then(|body| body.get("rows"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Hungama playlist {playlist_id} has no rows"),
                )
            })?;
        let mut entries = Vec::new();
        for row in rows {
            let Some(song_url) = row
                .get("data")
                .and_then(|data| data.get("misc"))
                .and_then(|misc| json_string(misc, "share"))
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let mut entry = native_url_result(song_url);
            entry.insert("ie_key", serde_json::json!("HungamaSong"));
            entries.push(entry);
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn hunga_call_api(
    context: &ExtractionContext,
    path: &str,
    content_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(&format!(
        "https://cpage.api.hungama.com/v2/page/content/{content_id}/{path}/detail"
    ));
    request.update_query(&[
        ("device".to_owned(), "web".to_owned()),
        ("platform".to_owned(), "a".to_owned()),
        ("storeId".to_owned(), "1".to_owned()),
    ]);
    let response = context.request(&request)?;
    let root = serde_json::from_slice::<serde_json::Value>(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Hungama API JSON for {content_id}: {error}"),
        )
    })?;
    Ok(root.get("data").cloned().unwrap_or(serde_json::Value::Null))
}

fn hunga_video_url(
    context: &ExtractionContext,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new("https://www.hungama.com/index.php");
    request.update_query(&[
        ("c".to_owned(), "common".to_owned()),
        ("m".to_owned(), "get_video_mdn_url".to_owned()),
    ]);
    request.set_method("POST").map_err(map_request_error)?;
    request
        .headers_mut()
        .set("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8");
    request.headers_mut().set("X-Requested-With", "XMLHttpRequest");
    request.set_data(Some(format!("content_id={video_id}").into_bytes()));
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Hungama video JSON for {video_id}: {error}"),
        )
    })
}

fn hunga_hls_format(
    stream_url: &str,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let extension = yt_dlp_core::determine_ext(Some(stream_url), "unknown");
    if extension != "m3u8" {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: Hungama video {video_id} returned a non-HLS stream that is not implemented in Rust"
            ),
        ));
    }
    Ok(serde_json::json!({
        "url": stream_url,
        "format_id": "hls",
        "protocol": "m3u8_native",
        "ext": "mp4",
    }))
}

fn hunga_tags(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let values = value?.as_array()?;
    let tags = values
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .map(serde_json::Value::String)
        .collect::<Vec<_>>();
    (!tags.is_empty()).then_some(serde_json::Value::Array(tags))
}

fn hunga_match_id(
    matcher: &Regex,
    url: &str,
    label: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, format!("{label} URL has no ID"))
        })
}
