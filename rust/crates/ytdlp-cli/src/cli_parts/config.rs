/// Parse lower-priority config files followed by the command line, matching
/// yt-dlp's last-source-wins option precedence.
pub fn parse_args_with_config_files(
    args: &[String],
    config_files: &[PathBuf],
) -> Result<ParseResult, CliError> {
    let mut combined = Vec::new();
    for path in config_files {
        combined.extend(read_config_file(path)?);
    }
    combined.extend(args.iter().cloned());
    parse_args(&combined)
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn push_existing(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if path.is_file() && !paths.contains(&path) {
        paths.push(path);
    }
}

/// Return default config files in yt-dlp's precedence order.
pub fn default_config_files() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            push_existing(&mut paths, directory.join("yt-dlp.conf"));
        }
    }
    if let Ok(directory) = std::env::current_dir() {
        push_existing(&mut paths, directory.join("yt-dlp.conf"));
    }

    let home = config_home();
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|path| path.join(".config")));
    if let Some(xdg) = xdg {
        push_existing(&mut paths, xdg.join("yt-dlp.conf"));
        push_existing(&mut paths, xdg.join("yt-dlp/config"));
        push_existing(&mut paths, xdg.join("yt-dlp/config.txt"));
    }
    if let Some(appdata) = std::env::var_os("appdata").or_else(|| std::env::var_os("APPDATA")) {
        let appdata = PathBuf::from(appdata);
        push_existing(&mut paths, appdata.join("yt-dlp.conf"));
        push_existing(&mut paths, appdata.join("yt-dlp/config"));
        push_existing(&mut paths, appdata.join("yt-dlp/config.txt"));
    }
    if let Some(home) = home {
        push_existing(&mut paths, home.join("yt-dlp.conf"));
        push_existing(&mut paths, home.join("yt-dlp.conf.txt"));
        push_existing(&mut paths, home.join(".yt-dlp/config"));
        push_existing(&mut paths, home.join(".yt-dlp/config.txt"));
    }
    push_existing(&mut paths, PathBuf::from("/etc/yt-dlp.conf"));
    push_existing(&mut paths, PathBuf::from("/etc/yt-dlp/config"));
    push_existing(&mut paths, PathBuf::from("/etc/yt-dlp/config.txt"));
    paths
}

fn resolve_config_location(path: &str) -> PathBuf {
    let home = config_home();
    let path = path
        .strip_prefix("~/")
        .and_then(|suffix| home.as_ref().map(|home| home.join(suffix)))
        .unwrap_or_else(|| PathBuf::from(path));
    if path.is_dir() {
        path.join("yt-dlp.conf")
    } else {
        path
    }
}

/// Parse command-line arguments together with the default and explicit config
/// locations. Explicit locations are required to exist; default candidates
/// are already filtered to existing files.
pub fn parse_configured_args(args: &[String]) -> Result<ParseResult, CliError> {
    let ignore_config = args.iter().any(|argument| {
        argument == "--ignore-config"
            || argument == "--no-config"
            || argument == "--ignore-config=true"
    });
    let mut files = if ignore_config {
        Vec::new()
    } else {
        default_config_files()
    };
    let mut custom_locations = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let (option, inline_value) = split_long_option(&args[index]);
        match option {
            "--config-locations" => {
                let value = inline_value
                    .clone()
                    .map_or_else(|| next_value(args, &mut index, option), Ok)?;
                custom_locations.push(resolve_config_location(&value));
            }
            "--no-config-locations" => custom_locations.clear(),
            _ => {}
        }
        index += 1;
    }
    for path in custom_locations {
        if !path.is_file() {
            return Err(CliError::new(format!(
                "config location {path:?} does not exist"
            )));
        }
        if !files.contains(&path) {
            files.push(path);
        }
    }
    parse_args_with_config_files(args, &files)
}
