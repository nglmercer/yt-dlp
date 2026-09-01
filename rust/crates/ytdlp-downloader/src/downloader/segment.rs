fn native_apply_extra_param_to_segment_url(
    request: &mut Request,
) -> Result<(), DownloadError> {
    let Some(value) = request.extensions().get("extra_param_to_segment_url") else {
        return Ok(());
    };
    let Some(value) = value.as_str() else {
        return Err(DownloadError::InvalidPlaylist(
            "extra_param_to_segment_url must be a string".to_owned(),
        ));
    };
    if value.is_empty() {
        return Ok(());
    }
    let mut url = Url::parse(request.url()).map_err(|error| {
        DownloadError::InvalidPlaylist(format!(
            "invalid segment URL while appending query parameters: {error}"
        ))
    })?;
    for (name, value) in url::form_urlencoded::parse(value.as_bytes()) {
        url.query_pairs_mut().append_pair(&name, &value);
    }
    request.set_url(url.to_string());
    Ok(())
}
