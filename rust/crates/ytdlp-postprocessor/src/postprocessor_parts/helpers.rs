fn resolve_ffmpeg(location: Option<&Path>) -> PathBuf {
    let Some(location) = location else {
        return PathBuf::from("ffmpeg");
    };
    if location.is_dir() {
        location.join("ffmpeg")
    } else {
        location.to_owned()
    }
}

/// Resolve the companion `ffprobe` executable, mirroring the probe half of
/// `--ffmpeg-location`: no location resolves `ffprobe` from `PATH`,
/// directories resolve both tools, and an explicit ffmpeg file leaves the
/// probe unresolved (callers then fall back to `ffmpeg -i`, exactly like
/// `probe_available == False` in Python).
fn resolve_ffprobe(location: Option<&Path>) -> Option<PathBuf> {
    match location {
        None => Some(PathBuf::from("ffprobe")),
        Some(dir) if dir.is_dir() => Some(dir.join("ffprobe")),
        Some(_) => None,
    }
}

fn ffmpeg_file_argument(path: &Path) -> String {
    let value = path.to_string_lossy();
    if value == "-" || value.starts_with("http://") || value.starts_with("https://") {
        value.into_owned()
    } else {
        format!("file:{value}")
    }
}

fn extra_args(
    options: &PostProcessOptions,
    processor_key: &str,
    executable_key: &str,
) -> Vec<String> {
    [
        format!("{processor_key}+{executable_key}"),
        executable_key.to_owned(),
        processor_key.to_owned(),
        "default-compat".to_owned(),
    ]
    .into_iter()
    .flat_map(|key| options.extra_args.get(&key).cloned().unwrap_or_default())
    .collect()
}

fn validate_extension(extension: &str) -> Result<(), PostProcessError> {
    if extension.is_empty() || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(PostProcessError::InvalidPath(format!(
            "invalid media extension: {extension:?}"
        )));
    }
    Ok(())
}

fn info_path(info: &InfoDict) -> Result<PathBuf, PostProcessError> {
    let value = info
        .get_str("filepath")
        .ok_or_else(|| PostProcessError::MissingField("filepath".to_owned()))?;
    if value.is_empty() {
        return Err(PostProcessError::InvalidPath("empty filepath".to_owned()));
    }
    Ok(PathBuf::from(value))
}

fn ensure_output_available(path: &Path, overwrite: bool) -> Result<(), PostProcessError> {
    if path.exists() && !overwrite {
        return Err(PostProcessError::OutputExists(path.to_owned()));
    }
    Ok(())
}

fn ensure_output_created(path: &Path, simulate: bool) -> Result<(), PostProcessError> {
    if !simulate && !path.is_file() {
        return Err(PostProcessError::MissingOutput(path.to_owned()));
    }
    Ok(())
}
