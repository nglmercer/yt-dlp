pub fn discover_runtimes(
    configured: &[(RuntimeKind, Option<PathBuf>)],
) -> Vec<Result<JavascriptRuntime, JavascriptError>> {
    configured
        .iter()
        .filter_map(
            |(kind, path)| match JavascriptRuntime::probe(*kind, path.as_deref()) {
                Ok(Some(runtime)) => Some(Ok(runtime)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            },
        )
        .collect()
}

/// Build the small wrapper used by the EJS provider protocol. Script bytes are
/// supplied by the caller so hash/version policy remains in the EJS layer.
pub fn build_ejs_script(library: &str, core: &str, input: &Value) -> String {
    format!(
        "{library}\nObject.assign(globalThis, lib);\n{core}\nconsole.log(JSON.stringify(jsc({input})));\n",
        input = input
    )
}

pub fn parse_json_output(output: &str) -> Result<Value, JavascriptError> {
    let line = output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| {
            JavascriptError::InvalidJson(serde_json::from_str::<Value>("").unwrap_err())
        })?;
    serde_json::from_str(line).map_err(JavascriptError::InvalidJson)
}

fn determine_runtime_path(kind: RuntimeKind, location: Option<&Path>) -> PathBuf {
    let Some(location) = location else {
        return PathBuf::from(kind.executable());
    };
    if location.is_dir() {
        location.join(kind.executable())
    } else {
        location.to_owned()
    }
}

fn parse_runtime_version(kind: RuntimeKind, output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let line = line.trim();
        let candidate = match kind {
            RuntimeKind::Deno => line.strip_prefix("deno "),
            RuntimeKind::Node => line.strip_prefix('v'),
            RuntimeKind::Bun => Some(line.strip_prefix('v').unwrap_or(line)),
            RuntimeKind::QuickJs => line.split_once("version ").map(|(_, version)| version),
        }?;
        let version = candidate.split_whitespace().next()?;
        (!parse_version_tuple(version).is_empty()).then(|| version.to_owned())
    })
}

fn parse_version_tuple(version: &str) -> Vec<u64> {
    version
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn unique_script_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "yt-dlp-rs-js-{}-{timestamp}.js",
        std::process::id()
    ))
}
