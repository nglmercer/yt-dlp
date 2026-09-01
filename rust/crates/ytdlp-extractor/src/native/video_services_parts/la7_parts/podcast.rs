fn la7_first_capture(html: &str, patterns: &[&str]) -> Option<String> {
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
    })
}

fn la7_first_fragment(html: &str, patterns: &[&str]) -> Option<String> {
    la7_first_capture(html, patterns)
        .map(|value| html_text_fragment(&value))
        .filter(|value| !value.is_empty())
}

fn la7_upload_date(value: &str) -> Option<String> {
    let dotted = Regex::new(r#"^(\d{1,2})[./](\d{1,2})[./](\d{4})$"#)
        .ok()
        .and_then(|matcher| matcher.captures(value.trim()).ok().flatten());
    if let Some(captures) = dotted {
        return Some(format!(
            "{}{:0>2}{:0>2}",
            captures.get(3)?.as_str(),
            captures.get(2)?.as_str(),
            captures.get(1)?.as_str()
        ));
    }
    date_digits(value)
}

fn la7_podcast_info(
    html: &str,
    page_url: &str,
    fallback_id: Option<&str>,
    ppn: Option<&str>,
) -> Result<InfoDict, ExtractorError> {
    let video_id = fallback_id
        .map(str::to_owned)
        .or_else(|| {
            la7_first_capture(
                html,
                &[r#"(?is)\bdata-nid\s*=\s*[\"'](\d+)[\"']"#],
            )
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "LA7 podcast episode has no media ID",
            )
        })?;
    let media_url = la7_first_capture(
        html,
        &[
            r#"(?is)\bsrc\s*:\s*[\"']([^\"']*mp3[^\"']*)[\"']"#,
            r#"(?is)\bdata-podcast\s*=\s*[\"']([^\"']*mp3[^\"']*)[\"']"#,
        ],
    )
    .map(|value| resolve_url(page_url, &value))
    .filter(|value| !value.is_empty())
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("LA7 podcast episode {video_id} has no MP3 source"),
        )
    })?;
    let mut title = la7_first_fragment(
        html,
        &[
            r#"(?is)<div[^>]+\bclass\s*=\s*[\"'][^\"']*\btitle\b[^\"']*[\"'][^>]*>(.*?)</div>"#,
            r#"(?is)<title\b[^>]*>(.*?)</title>"#,
            r#"(?is)\btitle\s*:\s*[\"'](.*?)[\"']"#,
        ],
    )
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("LA7 podcast episode {video_id} has no title"),
        )
    })?;
    let description = la7_first_fragment(
        html,
        &[
            r#"(?is)<div[^>]+\bclass\s*=\s*[\"'][^\"']*\bdescription\b[^\"']*[\"'][^>]*>(.*?)</div>"#,
            r#"(?is)<div[^>]+\bclass\s*=\s*[\"'][^\"']*\bbox-txt\b[^\"']*[\"'][^>]*>(.*?)</div>"#,
            r#"(?is)<div[^>]+\bclass\s*=\s*[\"'][^\"']*\bfield-content\b[^\"']*[\"'][^>]*>\s*<p[^>]*>(.*?)</p>"#,
        ],
    )
    .or_else(|| html_meta_value(html, "description").map(|value| html_text_fragment(&value)));
    let thumbnail = la7_first_capture(
        html,
        &[
            r#"(?is)<div[^>]+\bclass\s*=\s*[\"'][^\"']*\bpodcast-image\b[^\"']*[\"'][^>]*>.*?\bsrc\s*=\s*[\"']([^\"']+)[\"']"#,
            r#"(?is)<div[^>]+\bclass\s*=\s*[\"'][^\"']*\bcontainer-embed\b[^\"']*[\"'][^>]*>.*?url\(([^)]+)\)"#,
            r#"(?is)<div[^>]+\bclass\s*=\s*[\"'][^\"']*\bfield-content\b[^\"']*[\"'][^>]*>.*?\bsrc\s*=\s*[\"']([^\"']+)[\"']"#,
        ],
    )
    .map(|value| resolve_url(page_url, value.trim_matches(['\'', '"'])));
    let duration = la7_first_capture(
        html,
        &[r#"(?is)<span[^>]+\bclass\s*=\s*[\"'][^\"']*\b(?:durata|duration)\b[^\"']*[\"'][^>]*>([^<]+)</span>"#],
    )
    .and_then(|value| yt_dlp_core::parse_duration(value.trim()));
    let upload_date = la7_first_capture(
        html,
        &[r#"(?is)\bclass\s*=\s*[\"']data[\"']\s*>\s*(?:<span[^>]*>)?([\d./]+)"#],
    )
    .and_then(|value| la7_upload_date(&value));
    if let Some(ppn) = ppn {
        if ppn.eq_ignore_ascii_case(&title) {
            if let Some(date) = la7_first_capture(
                html,
                &[r#"(?is)\bclass\s*=\s*[\"']data[\"']\s*>\s*(?:<span[^>]*>)?([\d./]+)"#],
            ) {
                title = format!("{title} del {}", date.trim());
            }
        }
    }

    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert("title", serde_json::json!(title));
    info.insert(
        "formats",
        serde_json::json!([{
            "url": media_url,
            "format_id": "http-mp3",
            "ext": "mp3",
            "acodec": "mp3",
            "vcodec": "none",
        }]),
    );
    info.insert_if_some("description", description);
    info.insert_if_some("thumbnail", thumbnail);
    info.insert_if_some("duration", duration);
    info.insert_if_some("upload_date", upload_date);
    Ok(info)
}
