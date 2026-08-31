fn write_atomic(path: &Path, body: &[u8], overwrite: bool) -> Result<PathBuf, DownloadError> {
    if path.as_os_str().is_empty() || path == Path::new("-") {
        return Err(DownloadError::InvalidOutput(path.to_owned()));
    }
    if path.exists() && !overwrite {
        return Err(DownloadError::OutputExists(path.to_owned()));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let temporary = path.with_extension(format!(
        "{}.part",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("download")
    ));
    let mut file = File::create(&temporary)?;
    file.write_all(body)?;
    file.sync_all()?;
    drop(file);
    if overwrite && path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&temporary, path)?;
    Ok(path.to_owned())
}
