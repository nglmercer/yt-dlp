/// Native Epidemic Sound track catalog/audio extractor.
pub struct EpidemicSoundExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EpidemicSoundExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EpidemicSoundExtractor {
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
                "Epidemic Sound URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Epidemic Sound URL has no track ID",
                )
            })?;
        let is_sfx = captures.name("sfx").is_some();
        let endpoint = if is_sfx {
            format!("https://www.epidemicsound.com/json/track/kosmos-id/{video_id}")
        } else {
            format!("https://www.epidemicsound.com/json/track/{video_id}")
        };
        let data = context.get_json(&endpoint)?;
        let formats = epidemic_sound_formats(data.get("stems"));
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Epidemic Sound track {video_id} has no playable stems"),
            ));
        }
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let thumbnails = epidemic_sound_thumbnails(&data);
        let mut info = InfoDict::new();
        info.insert(
            "id",
            json_value_string(data.get("id"))
                .map_or_else(|| serde_json::json!(video_id), |value| serde_json::json!(value)),
        );
        info.insert_if_some(
            "display_id",
            json_string(&data, "publicSlug").filter(|value| !value.is_empty()),
        );
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert_if_some("alt_title", json_string(&data, "oldTitle"));
        info.insert_if_some("duration", json_f64(&data, "length"));
        info.insert_if_some(
            "timestamp",
            json_string(&data, "added")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "release_timestamp",
            json_string(&data, "releaseDate")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("categories", epidemic_sound_tags(data.get("genres")));
        info.insert_if_some("tags", epidemic_sound_tags(data.get("metadataTags")));
        if json_bool(&data, "isExplicit") == Some(true) {
            info.insert("age_limit", serde_json::json!(18));
        }
        if !thumbnails.is_empty() {
            info.insert_if_some(
                "thumbnail",
                thumbnails
                    .first()
                    .and_then(|value| value.get("url"))
                    .and_then(serde_json::Value::as_str),
            );
            info.insert("thumbnails", serde_json::Value::Array(thumbnails));
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
                .unwrap_or_else(|| serde_json::json!("mp3")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn epidemic_sound_formats(stems: Option<&serde_json::Value>) -> Vec<serde_json::Value> {
    let Some(stems) = stems.and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    stems
        .iter()
        .filter_map(|(stem_key, stem)| {
            let media_url = json_string(stem, "lqMp3Url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
            let format_name = json_string(stem, "format")
                .filter(|value| !value.is_empty())
                .or_else(|| json_string(stem, "stemType"))
                .unwrap_or(stem_key)
                .to_owned();
            let format_id = json_string(stem, "stemType")
                .filter(|value| !value.is_empty())
                .unwrap_or(&format_name)
                .to_owned();
            let extension = yt_dlp_core::determine_ext(Some(media_url), "mp3");
            let is_hls = extension.eq_ignore_ascii_case("m3u8");
            let mut format = serde_json::Map::new();
            format.insert("url".to_owned(), serde_json::json!(media_url));
            format.insert("format".to_owned(), serde_json::json!(format_name));
            format.insert("format_id".to_owned(), serde_json::json!(format_id));
            format.insert(
                "format_note".to_owned(),
                json_string(stem, "s3TrackId")
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value)),
            );
            format.insert(
                "protocol".to_owned(),
                serde_json::json!(if is_hls { "m3u8_native" } else { "http" }),
            );
            format.insert(
                "ext".to_owned(),
                serde_json::json!(if is_hls { "m4a" } else { extension.as_str() }),
            );
            format.insert("vcodec".to_owned(), serde_json::json!("none"));
            if format_name != "full" {
                format.insert("preference".to_owned(), serde_json::json!(-2));
            }
            Some(serde_json::Value::Object(format))
        })
        .collect()
}

fn epidemic_sound_tags(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let tags = value?
        .as_array()?
        .iter()
        .filter_map(|item| json_string(item, "tag").map(str::to_owned))
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>();
    (!tags.is_empty()).then_some(tags)
}

fn epidemic_sound_thumbnails(data: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut urls = Vec::new();
    for key in ["imageUrl", "cover"] {
        if let Some(thumbnail) = json_string(data, key)
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .map(str::to_owned)
        {
            if !urls.contains(&thumbnail) {
                urls.push(thumbnail);
            }
        }
    }
    if let Some(cover_art) = data.get("coverArt") {
        let base_url = json_string(cover_art, "baseUrl").unwrap_or_default();
        if let Some(sizes) = cover_art.get("sizes") {
            let size_values = sizes
                .as_array()
                .into_iter()
                .flatten()
                .chain(sizes.as_object().into_iter().flat_map(|values| values.values()))
                .filter_map(serde_json::Value::as_str);
            for size in size_values {
                let thumbnail = if size.starts_with("http://") || size.starts_with("https://") {
                    size.to_owned()
                } else {
                    format!("{base_url}{size}")
                };
                if (thumbnail.starts_with("http://") || thumbnail.starts_with("https://"))
                    && !urls.contains(&thumbnail)
                {
                    urls.push(thumbnail);
                }
            }
        }
    }
    urls.into_iter()
        .map(|url| epidemic_sound_thumbnail(&url))
        .collect()
}

fn epidemic_sound_thumbnail(url: &str) -> serde_json::Value {
    let mut thumbnail = serde_json::Map::new();
    thumbnail.insert("url".to_owned(), serde_json::json!(url));
    if let Some((width, height)) = Regex::new(r"(?i)(\d{2,5})x(\d{2,5})")
        .ok()
        .and_then(|matcher| matcher.captures(url).ok().flatten())
        .and_then(|captures| {
            Some((
                captures.get(1)?.as_str().parse::<i64>().ok()?,
                captures.get(2)?.as_str().parse::<i64>().ok()?,
            ))
        })
    {
        thumbnail.insert("width".to_owned(), serde_json::json!(width));
        thumbnail.insert("height".to_owned(), serde_json::json!(height));
    }
    serde_json::Value::Object(thumbnail)
}
