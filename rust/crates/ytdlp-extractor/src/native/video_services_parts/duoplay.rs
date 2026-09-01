/// Native Duoplay page/session/HLS extractor.
pub struct DuoplayExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DuoplayExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DuoplayExtractor {
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
        let telecast_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Duoplay URL has no ID")
            })?;
        let episode_id = url_query_value(url, "ep");
        let video_id = episode_id.as_ref().map_or_else(
            || telecast_id.clone(),
            |episode| format!("{telecast_id}_{episode}"),
        );
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let player = duoplay_player_tag(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Duoplay video {video_id} has no video-player element"),
            )
        })?;
        let manifest_url = duoplay_attribute(&player, "manifest-url")
            .map(|value| resolve_url(url, &value))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Duoplay video {video_id} has no manifest URL"),
                )
            })?;
        let mut session_request = Request::new("https://sts.postimees.ee/session/register");
        session_request.headers_mut().set("Accept", "application/json");
        session_request
            .headers_mut()
            .set("X-Original-URI", &manifest_url);
        let session_response = context.request(&session_request)?;
        let session_data: serde_json::Value = serde_json::from_slice(session_response.body())
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Duoplay session JSON for {video_id}: {error}"),
                )
            })?;
        let session = json_string(&session_data, "session")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Duoplay session registration for {video_id} has no session token"),
                )
            })?;
        let manifest_url = duoplay_session_manifest(&manifest_url, session, &video_id)?;
        let episode_data = duoplay_attribute(&player, ":episode")
            .map(|value| unescape_html_attribute(&value))
            .map(|value| serde_json::from_str::<serde_json::Value>(&value))
            .transpose()
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Duoplay episode JSON for {video_id}: {error}"),
                )
            })?
            .unwrap_or_else(|| serde_json::json!({}));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(telecast_id));
        info.insert("url", serde_json::json!(manifest_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": manifest_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        duoplay_insert_metadata(&mut info, &episode_data, episode_id.as_deref(), &video_id);
        Ok(ExtractorResult::single(info))
    }
}

fn duoplay_player_tag(html: &str) -> Option<String> {
    Regex::new(r"(?is)<video-player\b[^>]*>")
        .ok()?
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(0))
        .map(|value| value.as_str().to_owned())
}

fn duoplay_attribute(tag: &str, attribute: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)(?:^|\s){}\s*=\s*[\"']([^\"']*)"#,
        regex::escape(attribute)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(tag)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
}

fn duoplay_session_manifest(
    manifest_url: &str,
    session: &str,
    video_id: &str,
) -> Result<String, ExtractorError> {
    let mut manifest = url::Url::parse(manifest_url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Duoplay manifest URL for {video_id}: {error}"),
        )
    })?;
    manifest.query_pairs_mut().append_pair("s", session);
    Ok(manifest.to_string())
}

fn duoplay_insert_metadata(
    info: &mut InfoDict,
    episode: &serde_json::Value,
    episode_id: Option<&str>,
    video_id: &str,
) {
    let title = json_string(episode, "title")
        .or_else(|| json_string(episode, "subtitle"))
        .map(str::to_owned)
        .or_else(|| episode_id.map(|episode| format!("Episode {episode}")))
        .unwrap_or_else(|| video_id.to_owned());
    info.insert("title", serde_json::json!(title));
    info.insert_if_some("description", json_string(episode, "synopsis"));
    info.insert_if_some(
        "thumbnail",
        episode
            .get("images")
            .and_then(|images| json_string(images, "original")),
    );
    info.insert_if_some(
        "duration",
        json_f64(episode, "duration").or_else(|| json_f64(episode, "length")),
    );
    info.insert_if_some(
        "timestamp",
        json_string(episode, "airtime").and_then(duoplay_timestamp),
    );
    if let Some(cast) = json_string(episode, "cast") {
        let cast = cast
            .split(", ")
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if !cast.is_empty() {
            info.insert("cast", serde_json::json!(cast));
        }
    }
    info.insert_if_some("release_year", json_i64(episode, "year"));
    if json_string(episode, "category") != Some("movies") {
        info.insert_if_some("series", json_string(episode, "title"));
        info.insert_if_some("series_id", json_string(episode, "telecast_id"));
        info.insert_if_some("season_number", json_i64(episode, "season_id"));
        info.insert_if_some("episode", json_string(episode, "subtitle"));
        info.insert_if_some("episode_number", json_i64(episode, "episode_nr"));
        info.insert_if_some("episode_id", json_string(episode, "episode_id"));
    }
}

fn duoplay_timestamp(value: &str) -> Option<i64> {
    let value = value.trim();
    yt_dlp_core::parse_iso8601(value)
        .or_else(|| yt_dlp_core::parse_iso8601(&format!("{value}+02:00")))
}
