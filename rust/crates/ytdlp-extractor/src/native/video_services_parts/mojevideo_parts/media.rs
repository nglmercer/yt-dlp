fn mojevideo_formats(
    video_id: &str,
    video_id_dec: &str,
    video_expiry: &str,
    hashes: &[String],
) -> Result<Vec<serde_json::Value>, ExtractorError> {
    let qualities = [
        ("", 1, "normálna kvalita"),
        ("_lq", 0, "nízka kvalita"),
        ("_hd", 2, "HD-720p"),
        ("_fhd", 3, "FULL HD-1080p"),
        ("_2k", 4, "2K-1440p"),
    ];
    let mut formats = Vec::new();
    for ((suffix, quality, format_note), hash) in qualities.iter().zip(hashes) {
        let mut media_url = url::Url::parse(&format!(
            "https://cache01.mojevideo.sk/securevideos69/{video_id_dec}{suffix}.mp4"
        ))
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Mojevideo media URL for {video_id}: {error}"),
            )
        })?;
        media_url
            .query_pairs_mut()
            .append_pair("md5", hash)
            .append_pair("expires", video_expiry);
        formats.push(serde_json::json!({
            "format_id": format!("mp4-{quality}"),
            "quality": quality,
            "format_note": format_note,
            "url": media_url.to_string(),
            "ext": "mp4",
        }));
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Mojevideo video {video_id} has no signed formats"),
        ));
    }
    Ok(formats)
}
