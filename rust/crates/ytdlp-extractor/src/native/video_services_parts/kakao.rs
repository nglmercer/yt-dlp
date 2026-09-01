/// Native KakaoTV clip metadata and CDN rendition extractor.
pub struct KakaoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KakaoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KakaoExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "KakaoTV URL has no clip ID")
            })?;
        let api_url = format!("http://tv.kakao.com/api/v1/ft/playmeta/cliplink/{video_id}/");
        let mut metadata_request = Request::new(api_url);
        metadata_request.update_query(&[("player".to_owned(), "monet_html5".to_owned())]);
        metadata_request.update_query(&[("referer".to_owned(), url.to_owned())]);
        metadata_request.update_query(&[("uuid".to_owned(), String::new())]);
        metadata_request.update_query(&[("service".to_owned(), "kakao_tv".to_owned())]);
        metadata_request.update_query(&[("section".to_owned(), String::new())]);
        metadata_request.update_query(&[("dteType".to_owned(), "PC".to_owned())]);
        metadata_request.update_query(&[(
            "fields".to_owned(),
            "-* ,tid,clipLink,displayTitle,clip,title,description,channelId,createTime,duration,playCount,likeCount,commentCount,tagList,channel,name,clipChapterThumbnailList,thumbnailUrl,timeInSec,isDefault,videoOutputList,width,height,kbps,profile,label"
                .replace("-* ,", "-*,"),
        )]);
        let metadata_response = context.request(&metadata_request)?;
        let metadata = serde_json::from_slice::<serde_json::Value>(metadata_response.body())
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid KakaoTV metadata for {video_id}: {error}"),
                )
            })?;
        let clip_link = metadata.get("clipLink").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KakaoTV clip {video_id} has no clipLink metadata"),
            )
        })?;
        let clip = clip_link.get("clip").unwrap_or(clip_link);
        let title = json_string(clip, "title")
            .or_else(|| json_string(clip_link, "displayTitle"))
            .unwrap_or(&video_id);

        let mut formats = Vec::new();
        for rendition in clip
            .get("videoOutputList")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(profile) = json_string(rendition, "profile")
                .filter(|profile| !profile.eq_ignore_ascii_case("AUDIO"))
            else {
                continue;
            };
            let mut rendition_request = Request::new(format!(
                "https://tv.kakao.com/katz/v1/ft/cliplink/{video_id}/readyNplay"
            ));
            rendition_request.update_query(&[
                ("profile".to_owned(), profile.to_owned()),
                ("fields".to_owned(), "-*,code,message,url".to_owned()),
            ]);
            let rendition_response = context.request(&rendition_request)?;
            let rendition_data = serde_json::from_slice::<serde_json::Value>(
                rendition_response.body(),
            )
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid KakaoTV CDN response for {video_id}/{profile}: {error}"),
                )
            })?;
            let Some(media_url) = rendition_data
                .get("videoLocation")
                .and_then(|location| json_string(location, "url"))
                .filter(|media_url| {
                    media_url.starts_with("http://") || media_url.starts_with("https://")
                })
            else {
                continue;
            };
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": profile,
                "ext": "mp4",
                "protocol": "http",
            });
            if let Some(width) = json_i64(rendition, "width") {
                format["width"] = serde_json::json!(width);
            }
            if let Some(height) = json_i64(rendition, "height") {
                format["height"] = serde_json::json!(height);
            }
            if let Some(label) = json_string(rendition, "label") {
                format["format_note"] = serde_json::json!(label);
            }
            if let Some(filesize) = json_i64(rendition, "filesize") {
                format["filesize"] = serde_json::json!(filesize);
            }
            if let Some(tbr) = json_i64(rendition, "kbps") {
                format["tbr"] = serde_json::json!(tbr);
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KakaoTV clip {video_id} has no playable video formats"),
            ));
        }

        let mut thumbnails = Vec::new();
        if let Some(chapter_thumbnails) = clip
            .get("clipChapterThumbnailList")
            .and_then(serde_json::Value::as_array)
        {
            for thumbnail in chapter_thumbnails {
                let Some(thumbnail_url) = json_string(thumbnail, "thumbnailUrl") else {
                    continue;
                };
                let mut value = serde_json::json!({
                    "url": thumbnail_url,
                    "id": json_value_string(thumbnail.get("timeInSec")).unwrap_or_default(),
                });
                value["preference"] = serde_json::json!(
                    if json_bool(thumbnail, "isDefault").unwrap_or(false) {
                        -1
                    } else {
                        0
                    }
                );
                thumbnails.push(value);
            }
        }
        if let Some(thumbnail_url) = json_string(clip, "thumbnailUrl") {
            thumbnails.push(serde_json::json!({
                "url": thumbnail_url,
                "preference": 10,
            }));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", Some(title.to_owned()));
        info.insert_if_some("description", kakao_strip(json_string(clip, "description")));
        info.insert_if_some(
            "uploader",
            clip_link
                .get("channel")
                .and_then(|channel| json_string(channel, "name")),
        );
        info.insert_if_some("uploader_id", json_value_string(clip_link.get("channelId")));
        info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        info.insert_if_some(
            "timestamp",
            json_string(clip_link, "createTime").and_then(|value| parse_timestamp(value.to_owned())),
        );
        info.insert_if_some("duration", json_i64(clip, "duration"));
        info.insert_if_some("view_count", json_i64(clip, "playCount"));
        info.insert_if_some("like_count", json_i64(clip, "likeCount"));
        info.insert_if_some("comment_count", json_i64(clip, "commentCount"));
        if let Some(tags) = clip.get("tagList").and_then(serde_json::Value::as_array) {
            info.insert(
                "tags",
                serde_json::Value::Array(
                    tags.iter()
                        .filter_map(|tag| tag.as_str().map(|tag| serde_json::json!(tag)))
                        .collect(),
                ),
            );
        }
        info.insert("url", first.get("url").cloned().unwrap_or_default());
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn kakao_strip(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned)
}
