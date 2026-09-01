/// Native Eggs.mu single-song API extractor.
pub struct EggsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EggsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EggsExtractor {
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
        let song_id = eggs_match_id(&self.matcher, url, "Eggs song")?;
        let data = eggs_api_json(context, &format!("musics/{song_id}"), &song_id)?;
        eggs_music_result(&data)
    }
}

/// Native Eggs.mu artist playlist API extractor.
pub struct EggsArtistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EggsArtistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EggsArtistExtractor {
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
        let artist_id = eggs_match_id(&self.matcher, url, "Eggs artist")?;
        let artist = eggs_api_json(context, &format!("artists/{artist_id}"), &artist_id)?;
        let songs = eggs_api_json(
            context,
            &format!("artists/{artist_id}/musics"),
            &artist_id,
        )?;
        let songs = songs
            .get("data")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Eggs artist {artist_id} songs are not an array"),
                )
            })?;
        let mut entries = Vec::new();
        for song in songs {
            let result = eggs_music_result(song)?;
            match result {
                ExtractorResult::Single(info) => entries.push(info),
                ExtractorResult::Redirect { url, ie_key } => {
                    let mut info = native_url_result(&url);
                    info.insert_if_some("ie_key", ie_key);
                    entries.push(info);
                }
                ExtractorResult::Playlist { .. } => {
                    return Err(ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Eggs artist {artist_id} returned a nested playlist entry"),
                    ));
                }
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(artist_id));
        info.insert_if_some("title", json_string(&artist, "displayName").map(str::to_owned));
        info.insert_if_some("description", json_string(&artist, "profile").map(str::to_owned));
        info.insert_if_some(
            "thumbnail",
            json_string(&artist, "imageDataPath").map(str::to_owned),
        );
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn eggs_api_json(
    context: &ExtractionContext,
    endpoint: &str,
    display_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(format!("https://app-front-api.eggs.mu/v1/{endpoint}"));
    request.headers_mut().set("Accept", "*/*");
    request.headers_mut().set("apVersion", "8.2.00");
    request.headers_mut().set("deviceName", "Android");
    request.headers_mut().set("deviceId", "0000000000000000");
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Eggs API JSON for {display_id}: {error}"),
        )
    })
}

fn eggs_music_result(data: &serde_json::Value) -> Result<ExtractorResult, ExtractorError> {
    if let Some(youtube_url) = json_string(data, "youtubeUrl")
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
    {
        return Ok(ExtractorResult::Redirect {
            url: youtube_url.to_owned(),
            ie_key: Some("Youtube".to_owned()),
        });
    }
    let music_id = json_value_string(data.get("musicId"))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Eggs song record has no music ID",
            )
        })?;
    let title = json_string(data, "musicTitle")
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Eggs song {music_id} has no title"),
            )
        })?;
    let audio_url = json_string(data, "musicDataPath")
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(str::to_owned)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Eggs song {music_id} has no audio URL"),
            )
        })?;
    let artist = data.get("artist");
    let artist_name = artist.and_then(|artist| json_string(artist, "artistName"));
    let artist_display_name = artist.and_then(|artist| json_string(artist, "displayName"));
    let webpage_url = artist_name
        .zip(Some(music_id.as_str()))
        .map(|(artist_name, music_id)| format!("https://eggs.mu/artist/{artist_name}/song/{music_id}"));
    let extension = yt_dlp_core::determine_ext(Some(&audio_url), "m4a");
    let format = serde_json::json!({
        "url": audio_url,
        "format_id": "audio",
        "protocol": "http",
        "ext": extension,
        "vcodec": "none",
    });
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(music_id));
    info.insert("vcodec", serde_json::json!("none"));
    info.insert("extractor_key", serde_json::json!("eggs:single"));
    info.insert("extractor", serde_json::json!("eggs:single"));
    info.insert_if_some("title", Some(title));
    info.insert_if_some("url", Some(audio_url));
    info.insert_if_some("webpage_url", webpage_url);
    info.insert_if_some("uploader", artist_display_name.map(str::to_owned));
    info.insert_if_some(
        "uploader_id",
        artist.and_then(|artist| json_value_string(artist.get("artistId"))),
    );
    info.insert_if_some(
        "thumbnail",
        json_string(data, "imageDataPath").map(str::to_owned),
    );
    info.insert_if_some("view_count", json_i64(data, "numberOfMusicPlays"));
    info.insert_if_some("like_count", json_i64(data, "numberOfLikes"));
    info.insert_if_some("comment_count", json_i64(data, "numberOfComments"));
    info.insert_if_some("composers", eggs_string_list(data.get("composer")));
    info.insert_if_some("tags", eggs_string_list(data.get("tags")));
    info.insert_if_some(
        "timestamp",
        json_value_string(data.get("releaseDate")).and_then(parse_timestamp),
    );
    info.insert_if_some("artist", artist_display_name.map(str::to_owned));
    info.insert("ext", serde_json::json!(extension));
    info.insert("formats", serde_json::json!([format]));
    Ok(ExtractorResult::single(info))
}

fn eggs_string_list(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let values = match value? {
        serde_json::Value::String(value) => vec![value.to_owned()],
        serde_json::Value::Array(values) => values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    (!values.is_empty()).then_some(values)
}

fn eggs_match_id(
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
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("{label} URL has no ID"),
            )
        })
}
