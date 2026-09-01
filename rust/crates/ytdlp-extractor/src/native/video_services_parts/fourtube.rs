/// Native FourTube-family metadata/token extractor.
pub struct FourTubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FourTubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FourTubeExtractor {
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
                "FourTube-family URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "FourTube-family URL has no video ID",
                )
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned());
        let kind = captures.name("kind").map(|value| value.as_str());
        let (token_host, canonical_url, is_porn_tube) = fourtube_site(&self.descriptor.key)?;
        let page_url = if kind == Some("m") || display_id.is_none() {
            canonical_url.replace("{id}", &video_id)
        } else {
            url.to_owned()
        };
        let response = context.get(&page_url)?;
        let webpage = String::from_utf8_lossy(response.body());

        let mut metadata = FourTubeMetadata::default();
        if is_porn_tube {
            let video = fourtube_porn_tube_video(&webpage, &video_id)?;
            metadata.title = json_string(&video, "title").map(str::to_owned);
            metadata.thumbnail = json_string(&video, "masterThumb").map(str::to_owned);
            metadata.uploader = video
                .get("user")
                .and_then(|user| json_string(user, "username"))
                .map(str::to_owned)
                .or_else(|| {
                    video
                        .get("channel")
                        .and_then(|channel| json_string(channel, "name"))
                        .map(str::to_owned)
                });
            metadata.uploader_id = video
                .get("user")
                .and_then(|user| json_value_string(user.get("id")))
                .or_else(|| {
                    video
                        .get("channel")
                        .and_then(|channel| json_value_string(channel.get("id")))
                });
            metadata.channel = video
                .get("channel")
                .and_then(|channel| json_string(channel, "name"))
                .map(str::to_owned);
            metadata.channel_id = video
                .get("channel")
                .and_then(|channel| json_value_string(channel.get("id")));
            metadata.like_count = json_i64(&video, "likes");
            metadata.dislike_count = json_i64(&video, "dislikes");
            metadata.view_count = json_i64(&video, "playsQty");
            metadata.duration = json_f64(&video, "durationInSeconds");
            metadata.timestamp = json_string(&video, "publishedAt")
                .map(str::to_owned)
                .and_then(parse_timestamp);
            metadata.media_id = json_value_string(video.get("mediaId"));
            metadata.sources = video
                .get("encodings")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|encoding| {
                    json_value_string(encoding.get("height"))
                        .filter(|height| !height.is_empty())
                })
                .collect();
        } else {
            metadata.title = html_meta_value(&webpage, "name");
            metadata.timestamp = html_meta_value(&webpage, "uploadDate")
                .and_then(parse_timestamp);
            metadata.thumbnail = html_meta_value(&webpage, "thumbnailUrl");
            let (uploader_id, uploader) = fourtube_uploader(&webpage);
            metadata.uploader_id = uploader_id;
            metadata.uploader = uploader;
            metadata.categories = fourtube_categories(&webpage);
            metadata.view_count = fourtube_interaction_count(&webpage, "UserPlays");
            metadata.like_count = fourtube_interaction_count(&webpage, "UserLikes");
            metadata.duration = html_meta_value(&webpage, "duration")
                .and_then(|value| yt_dlp_core::parse_duration(&value));
            metadata.media_id = fourtube_media_id(&webpage);
            metadata.sources = fourtube_sources(&webpage);
        }

        let title = metadata.title.ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FourTube-family video {video_id} has no title"),
            )
        })?;
        if metadata.media_id.is_none() || metadata.sources.is_empty() {
            let (media_id, sources) =
                fourtube_player_parameters(&webpage, &page_url, &video_id, context)?;
            metadata.media_id = metadata.media_id.or(Some(media_id));
            if metadata.sources.is_empty() {
                metadata.sources = sources;
            }
        }
        let media_id = metadata.media_id.ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: FourTube-family video {video_id} has no native media ID; \
                     player bootstrap format is not implemented"
                ),
            )
        })?;
        if metadata.sources.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: FourTube-family video {video_id} has no native quality list; \
                     player bootstrap format is not implemented"
                ),
            ));
        }
        let formats = fourtube_token_formats(
            context,
            &page_url,
            &video_id,
            token_host,
            &media_id,
            &metadata.sources,
        )?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("thumbnail", metadata.thumbnail);
        info.insert_if_some("uploader", metadata.uploader);
        info.insert_if_some("uploader_id", metadata.uploader_id);
        info.insert_if_some("channel", metadata.channel);
        info.insert_if_some("channel_id", metadata.channel_id);
        info.insert_if_some("timestamp", metadata.timestamp);
        info.insert_if_some(
            "upload_date",
            metadata.timestamp.and_then(|timestamp| {
                chrono_like_date_digits(timestamp)
            }),
        );
        info.insert_if_some("duration", metadata.duration);
        info.insert_if_some("like_count", metadata.like_count);
        info.insert_if_some("dislike_count", metadata.dislike_count);
        info.insert_if_some("view_count", metadata.view_count);
        info.insert_if_some("categories", metadata.categories);
        info.insert("age_limit", serde_json::json!(18));
        Ok(ExtractorResult::single(info))
    }
}

#[derive(Default)]
struct FourTubeMetadata {
    title: Option<String>,
    thumbnail: Option<String>,
    uploader: Option<String>,
    uploader_id: Option<String>,
    channel: Option<String>,
    channel_id: Option<String>,
    timestamp: Option<i64>,
    duration: Option<f64>,
    like_count: Option<i64>,
    dislike_count: Option<i64>,
    view_count: Option<i64>,
    categories: Option<Vec<String>>,
    media_id: Option<String>,
    sources: Vec<String>,
}

fn fourtube_site(key: &str) -> Result<(&'static str, &'static str, bool), ExtractorError> {
    match key {
        "FourTubeIE" => Ok(("token.4tube.com", "https://www.4tube.com/videos/{id}/video", false)),
        "FuxIE" => Ok(("token.fux.com", "https://www.fux.com/video/{id}/video", false)),
        "PornTubeIE" => Ok(("tkn.porntube.com", "https://www.porntube.com/videos/video_{id}", true)),
        "PornerBrosIE" => Ok((
            "token.pornerbros.com",
            "https://www.pornerbros.com/videos/video_{id}",
            false,
        )),
        _ => Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: unsupported FourTube-family descriptor {key}"),
        )),
    }
}

fn fourtube_media_id(html: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<button\b[^>]*\bdata-id\s*=\s*["'](?P<id>\d+)["'][^>]*\bdata-quality\s*="#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
}

fn fourtube_sources(html: &str) -> Vec<String> {
    let Ok(matcher) = Regex::new(
        r#"(?is)<button\b[^>]*\bdata-quality\s*=\s*["']([^"']+)["'][^>]*>"#,
    ) else {
        return Vec::new();
    };
    matcher
        .captures_iter(html)
        .flatten()
        .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .filter(|value| !value.is_empty())
        .collect()
}

fn fourtube_uploader(html: &str) -> (Option<String>, Option<String>) {
    let Ok(anchor_matcher) =
        Regex::new(r#"(?is)<a\b[^>]*\bclass\s*=\s*["']item-to-subscribe["'][^>]*>"#)
    else {
        return (None, None);
    };
    let Ok(href_matcher) = Regex::new(r#"(?is)\bhref\s*=\s*["']([^"']+)["']"#) else {
        return (None, None);
    };
    let Ok(title_matcher) = Regex::new(r#"(?is)\btitle\s*=\s*["']([^"']+)["']"#) else {
        return (None, None);
    };
    anchor_matcher
        .captures_iter(html)
        .flatten()
        .find_map(|captures| {
            let anchor = captures.get(0)?.as_str();
            let href = href_matcher
                .captures(anchor)
                .ok()
                .flatten()
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str())?;
            let path = href.split('?').next()?.trim_end_matches('/');
            let parts = path.split('/').filter(|part| !part.is_empty()).collect::<Vec<_>>();
            let owner_index = parts.iter().rposition(|part| {
                matches!(*part, "channel" | "channels" | "user" | "users")
            })?;
            let uploader_id = parts.get(owner_index + 1)?.to_string();
            let uploader = title_matcher
                .captures(anchor)
                .ok()
                .flatten()
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().strip_prefix("Go to "))
                .and_then(|value| value.strip_suffix(" page"))
                .map(str::to_owned)?;
            Some((Some(uploader_id), Some(uploader)))
        })
        .unwrap_or((None, None))
}

fn fourtube_interaction_count(html: &str, interaction: &str) -> Option<i64> {
    let pattern = format!(
        r#"(?is)<meta\b[^>]*\bitemprop\s*=\s*["']interactionCount["'][^>]*\bcontent\s*=\s*["']{}:([0-9,]+)["']"#,
        regex::escape(interaction),
    );
    let matcher = Regex::new(&pattern).ok()?;
    let value = matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().replace(',', ""))?;
    value.parse().ok()
}

fn fourtube_categories(html: &str) -> Option<Vec<String>> {
    let section_matcher = Regex::new(
        r#"(?is)Categories\s*/\s*Tags.*?<ul\b[^>]*\bclass\s*=\s*["'][^"']*\blist\b[^"']*["'][^>]*>(.*?)</ul\s*>"#,
    )
    .ok()?;
    let section = section_matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str())?;
    let item_matcher = Regex::new(r#"(?is)<li\b[^>]*><a\b[^>]*>(.*?)</a\s*>"#).ok()?;
    let categories = item_matcher
        .captures_iter(section)
        .flatten()
        .filter_map(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    (!categories.is_empty()).then_some(categories)
}

fn fourtube_player_parameters(
    html: &str,
    page_url: &str,
    video_id: &str,
    context: &ExtractionContext,
) -> Result<(String, Vec<String>), ExtractorError> {
    let player_url = Regex::new(
        r#"(?is)<script\b[^>]*\bid\s*=\s*["']playerembed["'][^>]*\bsrc\s*=\s*["']([^"']+)["']"#,
    )
    .ok()
    .and_then(|matcher| matcher.captures(html).ok().flatten())
    .and_then(|captures| captures.get(1))
    .map(|value| resolve_url(page_url, value.as_str()))
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: FourTube-family video {video_id} requires an unrecognized \
                 player bootstrap script"
            ),
        )
    })?;
    let response = context.get(&player_url)?;
    let player_js = String::from_utf8_lossy(response.body());
    let params = Regex::new(
        r#"\$\.ajax\(url,\s*opts\);\s*\}\s*\}\)\s*\(([0-9,\[\] ]+)\)"#,
    )
    .ok()
    .and_then(|matcher| matcher.captures(&player_js).ok().flatten())
    .and_then(|captures| captures.get(1))
    .and_then(|value| parse_common_javascript_value(&format!("[{}]", value.as_str())))
    .and_then(|value| value.as_array().cloned())
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: FourTube-family video {video_id} has an unsupported \
                 player bootstrap parameter format"
            ),
        )
    })?;
    let media_id = params
        .first()
        .and_then(|value| json_value_string(Some(value)))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("FourTube-family video {video_id} player data has no media ID"),
            )
        })?;
    let sources = params
        .get(2)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| json_value_string(Some(value)))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    Ok((media_id, sources))
}

fn fourtube_token_formats(
    context: &ExtractionContext,
    page_url: &str,
    video_id: &str,
    token_host: &str,
    media_id: &str,
    sources: &[String],
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let token_url = format!(
        "https://{token_host}/{media_id}/{}/desktop",
        sources.join("+")
    );
    let parsed_url = url::Url::parse(page_url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("invalid FourTube-family page URL: {error}"),
        )
    })?;
    let origin = format!(
        "{}://{}",
        parsed_url.scheme(),
        parsed_url.host_str().unwrap_or_default()
    );
    let mut request = Request::new(token_url);
    request.headers_mut().set("Origin", origin);
    request.headers_mut().set("Referer", page_url);
    request.set_data(Some(Vec::new()));
    let response = context.request(&request)?;
    let tokens: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid FourTube-family token response for {video_id}: {error}"),
        )
    })?;
    sources.iter().map(|source| {
            let quality = source.parse::<i64>().map_err(|_| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!(
                        "FourTube-family quality {source} for video {video_id} is not numeric"
                    ),
                )
            })?;
            let token = tokens
                .get(source)
                .and_then(|value| json_string(value, "token"))
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!(
                            "FourTube-family token response has no token for {source}p \
                             video {video_id}"
                        ),
                    )
                })?;
            let format_url = token.to_owned();
            let extension =
                yt_dlp_core::determine_ext(Some(&format_url), "mp4");
            let resolution = format!("{source}p");
            Ok(serde_json::json!({
                "url": format_url,
                "format_id": resolution,
                "resolution": resolution,
                "quality": quality,
                "ext": extension,
            }))
        })
        .collect()
}

fn fourtube_porn_tube_video(
    html: &str,
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let encoded = Regex::new(
        r#"(?is)\bINITIALSTATE\s*=\s*(["'])(?P<value>(?:(?!\1).)+)\1"#,
    )
    .ok()
    .and_then(|matcher| matcher.captures(html).ok().flatten())
    .and_then(|captures| captures.name("value"))
    .map(|value| value.as_str().to_owned())
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: PornTube video {video_id} has no INITIALSTATE payload"),
        )
    })?;
    let decoded = fourtube_base64_decode(&encoded)
        .and_then(|value| String::from_utf8(value).ok())
        .map(|value| percent_decode(&value))
        .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
        .and_then(|value| value.get("page").and_then(|page| page.get("video")).cloned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: PornTube video {video_id} has an unsupported INITIALSTATE payload"
                ),
            )
        })?;
    Ok(decoded)
}

fn fourtube_base64_decode(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in value.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            break;
        }
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(digit);
        bits = bits.saturating_add(6);
        while bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            if bits > 0 {
                buffer &= (1 << bits) - 1;
            } else {
                buffer = 0;
            }
        }
    }
    Some(output)
}

fn chrono_like_date_digits(timestamp: i64) -> Option<String> {
    let days = timestamp.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days)?;
    Some(format!("{year:04}{month:02}{day:02}"))
}

fn civil_from_days(days: i64) -> Option<(i64, i64, i64)> {
    let z = days.checked_add(719_468)?;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year >= 0).then_some((year, month, day))
}
