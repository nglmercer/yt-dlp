impl InfoExtractor for BlerpExtractor {
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
        let audio_id = last_path_segment(url)?;
        let payload = serde_json::json!({
            "operationName": "webBitePageGetBite",
            "variables": {"_id": audio_id},
            "query": "query webBitePageGetBite($_id: MongoID!) { web { biteById(_id: $_id) { _id title userKeywords ownerObject { _id username } audio { mp3 { url } } } } }",
        });
        let mut request = Request::new("https://api.blerp.com/graphql");
        request.set_method("POST").map_err(map_request_error)?;
        request
            .headers_mut()
            .set("Content-Type", "application/json");
        request.set_data(Some(serde_json::to_vec(&payload).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("could not encode Blerp GraphQL request: {error}"),
            )
        })?));
        let response = context.request(&request)?;
        let response: serde_json::Value =
            serde_json::from_slice(response.body()).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Blerp GraphQL response: {error}"),
                )
            })?;
        let bite = response
            .get("data")
            .and_then(|data| data.get("web"))
            .and_then(|web| web.get("biteById"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Blerp GraphQL response has no bite",
                )
            })?;
        let media_url = bite
            .get("audio")
            .and_then(|audio| audio.get("mp3"))
            .and_then(|mp3| mp3.get("url"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Blerp response has no MP3 URL",
                )
            })?;
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(json_string(bite, "_id").unwrap_or(&audio_id)),
        );
        info.insert(
            "title",
            serde_json::json!(json_string(bite, "title").unwrap_or(&audio_id)),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "mp3",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        if let Some(owner) = bite.get("ownerObject") {
            info.insert_if_some("uploader", json_string(owner, "username"));
            info.insert_if_some("uploader_id", json_string(owner, "_id"));
        }
        if let Some(tags) = bite
            .get("userKeywords")
            .and_then(serde_json::Value::as_array)
        {
            info.insert("tags", serde_json::Value::Array(tags.clone()));
        }
        Ok(ExtractorResult::single(info))
    }
}

fn audius_data<'a>(
    response: &'a serde_json::Value,
) -> Result<&'a serde_json::Value, ExtractorError> {
    response.get("data").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Audius API response has no data field",
        )
    })
}

fn json_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .unwrap_or_else(|| value.to_string())
    })
}

impl InfoExtractor for AudiusExtractor {
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
        let hosts_response = context.get_json("https://api.audius.co/")?;
        let hosts = audius_data(&hosts_response)?;
        let host = hosts
            .as_array()
            .and_then(|hosts| hosts.iter().find_map(|host| host.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Audius host discovery returned no API hosts",
                )
            })?
            .trim_end_matches('/')
            .to_owned();
        let track_response = if self.descriptor.key == "AudiusTrackIE" {
            let track_id = url
                .rsplit('/')
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::InvalidUrl,
                        "Audius track URL has no track ID",
                    )
                })?;
            context.get_json(&format!("{host}/v1/tracks/{track_id}"))?
        } else {
            let mut resolve_request = Request::new(format!("{host}/v1/resolve"));
            resolve_request.update_query(&[("url".to_owned(), url.to_owned())]);
            context.get_json(resolve_request.url())?
        };
        let track_data = audius_data(&track_response)?;
        let track_id = json_value_string(track_data.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Audius response has no track ID",
            )
        })?;
        let title = json_string(track_data, "title")
            .map(str::to_owned)
            .unwrap_or_else(|| track_id.clone());
        let stream_url = format!("{host}/v1/tracks/{track_id}/stream");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id));
        info.insert("title", serde_json::json!(title.clone()));
        info.insert("track", serde_json::json!(title));
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "stream",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some("description", json_string(track_data, "description"));
        info.insert_if_some("duration", json_f64(track_data, "duration"));
        info.insert_if_some("genre", json_string(track_data, "genre"));
        for (name, source) in [
            ("view_count", "play_count"),
            ("like_count", "favorite_count"),
            ("repost_count", "repost_count"),
        ] {
            if let Some(value) = track_data.get(source) {
                info.insert(name, value.clone());
            }
        }
        if let Some(artist) = track_data
            .get("user")
            .and_then(|user| user.get("name"))
            .and_then(serde_json::Value::as_str)
        {
            info.insert("artist", serde_json::json!(artist));
        }
        if let Some(artwork) = track_data
            .get("artwork")
            .and_then(serde_json::Value::as_object)
        {
            let thumbnails = artwork
                .iter()
                .filter_map(|(quality, value)| {
                    value.as_str().map(|url| {
                        serde_json::json!({
                            "id": quality,
                            "url": url,
                        })
                    })
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for BreitbartExtractor {
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
        let video_id = path_segment_after(url, "v")?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let manifest_url = format!("https://cdn.jwplayer.com/manifests/{video_id}.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title").unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("url", serde_json::json!(manifest_url));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": manifest_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}
