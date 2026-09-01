const KUWO_ANTI_SERVER: &str = "http://antiserver.kuwo.cn/anti.s";
const KUWO_BASE_FORMATS: [KuwoFormatSpec; 6] = [
    KuwoFormatSpec {
        format_id: "ape",
        extension: "ape",
        bitrate: None,
        abr: None,
        quality: 100,
    },
    KuwoFormatSpec {
        format_id: "mp3-320",
        extension: "mp3",
        bitrate: Some("320kmp3"),
        abr: Some(320),
        quality: 80,
    },
    KuwoFormatSpec {
        format_id: "mp3-192",
        extension: "mp3",
        bitrate: Some("192kmp3"),
        abr: Some(192),
        quality: 70,
    },
    KuwoFormatSpec {
        format_id: "mp3-128",
        extension: "mp3",
        bitrate: Some("128kmp3"),
        abr: Some(128),
        quality: 60,
    },
    KuwoFormatSpec {
        format_id: "wma",
        extension: "wma",
        bitrate: None,
        abr: None,
        quality: 20,
    },
    KuwoFormatSpec {
        format_id: "aac",
        extension: "aac",
        bitrate: None,
        abr: Some(48),
        quality: 10,
    },
];

#[derive(Clone, Copy)]
struct KuwoFormatSpec {
    format_id: &'static str,
    extension: &'static str,
    bitrate: Option<&'static str>,
    abr: Option<i64>,
    quality: i64,
}

fn kuwo_match_id(
    matcher: &Regex,
    url: &str,
    description: &str,
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
                format!("Kuwo {description} URL has no ID"),
            )
        })
}

fn kuwo_http_url(value: &str) -> Option<String> {
    let value = value.trim();
    (value.starts_with("http://") || value.starts_with("https://"))
        .then(|| value.to_owned())
}

fn kuwo_page(
    context: &ExtractionContext,
    url: &str,
    _description: &str,
) -> Result<(String, String), ExtractorError> {
    let response = context.get(url)?;
    Ok((
        String::from_utf8_lossy(response.body()).into_owned(),
        response.url().to_owned(),
    ))
}

fn kuwo_text_request(
    context: &ExtractionContext,
    endpoint: &str,
    description: &str,
) -> Result<String, ExtractorError> {
    let response = context.get(endpoint)?;
    if response.status() >= 400 {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Network,
            format!("HTTP {} while extracting Kuwo {description}", response.status()),
        ));
    }
    Ok(String::from_utf8_lossy(response.body()).trim().to_owned())
}

fn kuwo_formats(
    context: &ExtractionContext,
    song_id: &str,
    tolerate_ip_deny: bool,
    include_mv: bool,
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let mut specs = KUWO_BASE_FORMATS.to_vec();
    if include_mv {
        specs.extend([
            KuwoFormatSpec {
                format_id: "mkv",
                extension: "mkv",
                bitrate: None,
                abr: None,
                quality: 250,
            },
            KuwoFormatSpec {
                format_id: "mp4",
                extension: "mp4",
                bitrate: None,
                abr: None,
                quality: 200,
            },
        ]);
    }
    let mut formats = Vec::new();
    for spec in specs {
        let mut request = Request::new(KUWO_ANTI_SERVER);
        request.update_query(&[
            ("format".to_owned(), spec.extension.to_owned()),
            (
                "br".to_owned(),
                spec.bitrate.unwrap_or_default().to_owned(),
            ),
            ("rid".to_owned(), format!("MUSIC_{song_id}")),
            ("type".to_owned(), "convert_url".to_owned()),
            ("response".to_owned(), "url".to_owned()),
        ]);
        let song_url = kuwo_text_request(context, request.url(), spec.format_id)?;
        if song_url == "IPDeny" {
            if !tolerate_ip_deny {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: Kuwo song {song_id} is blocked in this region (anti-server IPDeny)"
                    ),
                ));
            }
            continue;
        }
        let Some(song_url) = kuwo_http_url(&song_url) else {
            continue;
        };
        let mut format = serde_json::json!({
            "url": song_url,
            "format_id": spec.format_id,
            "format": spec.format_id,
            "quality": spec.quality,
        });
        if let Some(abr) = spec.abr {
            format["abr"] = serde_json::json!(abr);
        }
        formats.push(format);
    }
    Ok(formats)
}

fn kuwo_song_name(html: &str) -> Option<String> {
    let matcher =
        Regex::new(r#"(?is)<p[^>]+id\s*=\s*["']lrcName["'][^>]*>([^<]+)</p>"#).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn kuwo_singer_name(html: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<a[^>]+href\s*=\s*["']http://www\.kuwo\.cn/artist/content\?name=([^"']+)["']"#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| percent_decode(value.as_str()))
        .map(|value| value.strip_prefix("歌手").unwrap_or(&value).to_owned())
        .filter(|value| !value.is_empty())
}

fn kuwo_lyrics(html: &str) -> Option<String> {
    let lyrics = html_element_by_id(html, "lrcContent")
        .map(|value| html_text_fragment(&value))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && value != "暂无")?;
    Some(lyrics)
}

fn kuwo_album_id(html: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<a[^>]+href\s*=\s*["']http://www\.kuwo\.cn/album/(\d+)/["']"#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn kuwo_publish_date(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"发行时间：([0-9]{4}-[0-9]{2}-[0-9]{2})"#).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().replace('-', ""))
}

fn kuwo_intro(html: &str, title: &str) -> Option<String> {
    let intro = html_element_by_id(html, "intro")
        .map(|value| html_text_fragment(&value))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())?;
    let prefix = format!("{title}简介：");
    let intro = intro
        .strip_prefix(&prefix)
        .unwrap_or(&intro)
        .trim()
        .to_owned();
    (!intro.is_empty() && intro != "暂无").then_some(intro)
}

fn kuwo_song_url(song_id: &str) -> String {
    format!("http://www.kuwo.cn/yinyue/{song_id}/")
}

fn kuwo_entry(song_url: &str) -> InfoDict {
    let mut entry = native_url_result(song_url);
    entry.insert("ie_key", serde_json::json!("Kuwo"));
    entry
}

fn kuwo_absolute_url(base_url: &str, path: &str) -> String {
    url::Url::parse(base_url)
        .ok()
        .and_then(|base| base.join(path).ok())
        .map(|value| value.to_string())
        .unwrap_or_else(|| path.to_owned())
}
