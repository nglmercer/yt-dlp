/// Native Netzkino Next.js/CMS movie extractor.
pub struct NetzkinoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NetzkinoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NetzkinoExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Netzkino URL has no movie ID")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let next_data = html_script_json(&html, "__NEXT_DATA__")?;
        let movie = next_data
            .get("props")
            .and_then(|props| props.get("__dehydratedState"))
            .and_then(|state| state.get("queries"))
            .and_then(serde_json::Value::as_array)
            .and_then(|queries| {
                queries.iter().find_map(|query| {
                    let data = query
                        .get("state")
                        .and_then(|state| state.get("data"))
                        .and_then(|data| data.get("data"))?;
                    (json_string(data, "__typename") == Some("CmsMovie")).then_some(data)
                })
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Netzkino movie {video_id} is missing from page state"),
                )
            })?;
        let raw_media_url = movie
            .get("videoSource")
            .and_then(|source| json_string(source, "pmdUrl"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Netzkino movie {video_id} has no media URL"),
                )
            })?;
        let media_url = resolve_url(
            "https://pmd.netzkino-seite.netzkino.de/",
            raw_media_url,
        );
        let detected_ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4").to_ascii_lowercase();
        let (format_id, protocol, format_ext) = match detected_ext.as_str() {
            "m3u8" => ("hls", "m3u8_native", "mp4"),
            "mpd" => ("dash", "http_dash_segments", "mp4"),
            _ => ("http", "http", detected_ext.as_str()),
        };
        let title = json_string(movie, "originalTitle")
            .map(html_text_fragment)
            .filter(|value| !value.is_empty());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title.clone());
        info.insert_if_some("alt_title", title);
        info.insert_if_some("age_limit", json_i64(movie, "fskRating"));
        info.insert_if_some(
            "description",
            json_string(movie, "longSynopsis")
                .map(html_text_fragment)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some("duration", json_i64(movie, "runtimeInSeconds"));
        info.insert_if_some("location", json_string(movie, "productionCountry"));
        info.insert_if_some("release_year", json_i64(movie, "productionYear"));
        info.insert_if_some(
            "thumbnail",
            movie
                .get("coverImage")
                .and_then(|image| json_string(image, "masterUrl"))
                .map(|value| resolve_url(url, value)),
        );
        netzkino_insert_names(&mut info, "cast", movie, &["cast"]);
        netzkino_insert_names(&mut info, "creators", movie, &["directors", "writers"]);
        netzkino_insert_names(&mut info, "categories", movie, &["categories"]);
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(format_ext));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": format_id,
                "protocol": protocol,
                "ext": format_ext,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

fn netzkino_insert_names(
    info: &mut InfoDict,
    target: &str,
    movie: &serde_json::Value,
    sources: &[&str],
) {
    let mut values = Vec::new();
    for source in sources {
        let Some(nodes) = movie
            .get(*source)
            .and_then(|value| value.get("nodes"))
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for node in nodes {
            let Some(name) = node
                .get("person")
                .and_then(|person| json_string(person, "name"))
                .map(html_text_fragment)
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    node.get("category")
                        .and_then(|category| json_string(category, "title"))
                        .map(html_text_fragment)
                        .filter(|value| !value.is_empty())
                })
            else {
                continue;
            };
            values.push(name);
        }
    }
    if !values.is_empty() {
        info.insert(target, serde_json::Value::Array(
            values
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        ));
    }
}
