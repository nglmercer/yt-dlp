/// Native Jamendo track extractor.
pub struct JamendoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl JamendoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for JamendoExtractor {
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
                "Jamendo track URL did not match its native pattern",
            )
        })?;
        let track_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Jamendo track has no ID")
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned());
        let track = jamendo_call_api(context, "track", &track_id)?;
        let track_name = jamendo_string(track.get("name")).unwrap_or_else(|| track_id.clone());
        let artist = jamendo_optional_api(
            context,
            "artist",
            json_value_string(track.get("artistId")).as_deref(),
        );
        let album = jamendo_optional_api(
            context,
            "album",
            json_value_string(track.get("albumId")).as_deref(),
        );
        let formats = [
            ("mp31", "mp3l", "mp3"),
            ("mp32", "mp3d", "mp3"),
            ("ogg1", "ogg", "ogg"),
            ("flac", "flac", "flac"),
        ]
        .into_iter()
        .enumerate()
        .map(|(quality, (format_id, subdomain, extension))| {
            serde_json::json!({
                "url": format!("https://{subdomain}.jamendo.com/?trackid={track_id}&format={format_id}&from=app-97dab294"),
                "format_id": format_id,
                "ext": extension,
                "quality": quality,
                "vcodec": "none",
            })
        })
        .collect::<Vec<_>>();

        let thumbnails = jamendo_thumbnails(context, &track_id, &track);
        let timestamp = jamendo_integer(track.get("dateCreated"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id));
        info.insert_if_some("display_id", display_id);
        info.insert_if_some("title", jamendo_string(track.get("name")));
        info.insert("track", serde_json::json!(track_name));
        info.insert_if_some("description", jamendo_string(track.get("description")));
        info.insert_if_some("duration", jamendo_integer(track.get("duration")));
        info.insert_if_some(
            "artist",
            artist
                .as_ref()
                .and_then(|artist| jamendo_string(artist.get("name"))),
        );
        info.insert_if_some(
            "album",
            album
                .as_ref()
                .and_then(|album| jamendo_string(album.get("name"))),
        );
        info.insert("formats", serde_json::Value::Array(formats.clone()));
        info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        info.insert_if_some(
            "license",
            track
                .get("licenseCC")
                .and_then(serde_json::Value::as_array)
                .map(|license| {
                    license
                        .iter()
                        .filter_map(|part| part.as_str())
                        .collect::<Vec<_>>()
                        .join("-")
                })
                .filter(|license| !license.is_empty()),
        );
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some("upload_date", timestamp.map(jamendo_upload_date));
        let stats = track.get("stats");
        info.insert_if_some(
            "view_count",
            jamendo_integer(stats.and_then(|stats| stats.get("listenedAll"))),
        );
        info.insert_if_some(
            "like_count",
            jamendo_integer(stats.and_then(|stats| stats.get("favorited"))),
        );
        info.insert_if_some(
            "average_rating",
            jamendo_integer(stats.and_then(|stats| stats.get("averageNote"))),
        );
        info.insert_if_some("tags", jamendo_tags(track.get("tags")));
        if let Some(first) = formats.first() {
            info.insert("url", first.get("url").cloned().unwrap_or_default());
            info.insert("ext", first.get("ext").cloned().unwrap_or_default());
        }
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn jamendo_tags(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let tags = value?.as_array()?;
    let tags = tags
        .iter()
        .filter_map(|tag| jamendo_string(tag.get("name")))
        .collect::<Vec<_>>();
    (!tags.is_empty()).then_some(tags)
}

fn jamendo_thumbnails(
    context: &ExtractionContext,
    track_id: &str,
    track: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let Some(covers) = track.get("cover").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    let mut seen_urls = Vec::new();
    let mut thumbnails = Vec::new();
    for cover_set in covers.values() {
        let Some(cover_set) = cover_set.as_object() else {
            continue;
        };
        for (cover_id, cover_value) in cover_set {
            let Some(cover_url) = cover_value.as_str().filter(|value| !value.is_empty()) else {
                continue;
            };
            if seen_urls.iter().any(|value| value == cover_url) {
                continue;
            }
            seen_urls.push(cover_url.to_owned());
            let mut request = Request::new(cover_url);
            if request.set_method("HEAD").is_err() {
                continue;
            }
            let Ok(response) = context.request(&request) else {
                continue;
            };
            let size = cover_id
                .strip_prefix("size")
                .and_then(|value| value.parse::<i64>().ok());
            let extension = jamendo_thumbnail_extension(&response, cover_url);
            let mut thumbnail = serde_json::json!({
                "id": cover_id,
                "ext": extension,
                "url": cover_url,
            });
            if let Some(size) = size {
                thumbnail["width"] = serde_json::json!(size);
                thumbnail["height"] = serde_json::json!(size);
            }
            thumbnails.push(thumbnail);
        }
    }
    let _ = track_id;
    thumbnails
}
