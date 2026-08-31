//! JavaScript runtime adapters used by the native extractor runtime boundary.
//!
//! The adapter owns executable discovery, version probing, stdin/stdout
//! execution, and the QuickJS temporary-file difference.  Challenge-solving
//! scripts remain an independent input so the runtime layer can be tested
//! without bundling a particular EJS release into the Rust binary.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeKind {
    Deno,
    Node,
    Bun,
    QuickJs,
}

impl RuntimeKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Deno => "deno",
            Self::Node => "node",
            Self::Bun => "bun",
            Self::QuickJs => "quickjs",
        }
    }

    pub fn executable(self) -> &'static str {
        match self {
            Self::Deno => "deno",
            Self::Node => "node",
            Self::Bun => "bun",
            Self::QuickJs => "qjs",
        }
    }

    fn minimum_supported(self) -> &'static [u64] {
        match self {
            Self::Deno => &[2, 3, 0],
            Self::Node => &[22, 0, 0],
            Self::Bun => &[1, 2, 11],
            Self::QuickJs => &[2023, 12, 9],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub kind: RuntimeKind,
    pub name: String,
    pub path: PathBuf,
    pub version: String,
    pub version_tuple: Vec<u64>,
    pub supported: bool,
}

#[derive(Debug)]
pub enum JavascriptError {
    Io(std::io::Error),
    Unavailable {
        kind: RuntimeKind,
        path: PathBuf,
    },
    InvalidVersion {
        kind: RuntimeKind,
        output: String,
    },
    Unsupported(RuntimeInfo),
    Failed {
        runtime: RuntimeInfo,
        status: Option<i32>,
        stderr: String,
    },
    InvalidJson(serde_json::Error),
}

impl fmt::Display for JavascriptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Unavailable { kind, path } => {
                write!(
                    formatter,
                    "{} runtime is unavailable at {:?}",
                    kind.name(),
                    path
                )
            }
            Self::InvalidVersion { kind, output } => {
                write!(
                    formatter,
                    "could not parse {} runtime version from {output:?}",
                    kind.name()
                )
            }
            Self::Unsupported(info) => write!(
                formatter,
                "{} runtime version {} is not supported",
                info.name, info.version
            ),
            Self::Failed {
                runtime,
                status,
                stderr,
            } => write!(
                formatter,
                "{} failed with status {:?}: {}",
                runtime.name,
                status,
                stderr.trim()
            ),
            Self::InvalidJson(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for JavascriptError {}

impl From<std::io::Error> for JavascriptError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeOptions {
    pub allow_network: bool,
    pub no_check_certificate: bool,
    pub jitless: bool,
    pub environment: BTreeMap<String, String>,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            allow_network: false,
            no_check_certificate: false,
            jitless: false,
            environment: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInvocation {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl RuntimeInvocation {
    pub fn argv(&self) -> Vec<String> {
        std::iter::once(self.program.to_string_lossy().into_owned())
            .chain(self.args.iter().cloned())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavascriptOutput {
    pub runtime: RuntimeInfo,
    pub invocation: RuntimeInvocation,
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

#[derive(Debug, Clone)]
pub struct JavascriptRuntime {
    info: RuntimeInfo,
}

impl JavascriptRuntime {
    /// Probe one configured runtime. An unavailable executable returns
    /// `Ok(None)`, matching yt-dlp's ability to try the next provider.
    pub fn probe(
        kind: RuntimeKind,
        location: Option<&Path>,
    ) -> Result<Option<Self>, JavascriptError> {
        let path = determine_runtime_path(kind, location);
        let args = if kind == RuntimeKind::QuickJs {
            vec!["--help"]
        } else {
            vec!["--version"]
        };
        let output = match Command::new(&path).args(&args).output() {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let version = parse_runtime_version(kind, &combined).ok_or_else(|| {
            JavascriptError::InvalidVersion {
                kind,
                output: combined.clone(),
            }
        })?;
        let version_tuple = parse_version_tuple(&version);
        let info = RuntimeInfo {
            kind,
            name: if kind == RuntimeKind::QuickJs && combined.contains("QuickJS-ng") {
                "quickjs-ng".to_owned()
            } else {
                kind.name().to_owned()
            },
            path,
            version,
            supported: version_tuple.as_slice() >= kind.minimum_supported(),
            version_tuple,
        };
        Ok(Some(Self { info }))
    }

    pub fn from_info(info: RuntimeInfo) -> Result<Self, JavascriptError> {
        if !info.supported {
            return Err(JavascriptError::Unsupported(info));
        }
        Ok(Self { info })
    }

    pub fn info(&self) -> &RuntimeInfo {
        &self.info
    }

    pub fn invocation(&self, options: &RuntimeOptions) -> RuntimeInvocation {
        let mut args = match self.info.kind {
            RuntimeKind::Deno => {
                let mut args = vec![
                    "run".to_owned(),
                    "--ext=js".to_owned(),
                    "--no-code-cache".to_owned(),
                    "--no-prompt".to_owned(),
                    "--no-lock".to_owned(),
                    "--node-modules-dir=none".to_owned(),
                    "--no-config".to_owned(),
                ];
                if !options.allow_network {
                    args.extend([
                        "--no-remote".to_owned(),
                        "--no-npm".to_owned(),
                        "--cached-only".to_owned(),
                    ]);
                }
                if options.no_check_certificate {
                    args.push("--unsafely-ignore-certificate-errors".to_owned());
                }
                if options.jitless {
                    args.push("--v8-flags=--jitless".to_owned());
                }
                args.push("-".to_owned());
                args
            }
            RuntimeKind::Node => {
                let mut args = Vec::new();
                if options.jitless {
                    args.push("--v8-flags=--jitless".to_owned());
                }
                args.push("-".to_owned());
                args
            }
            RuntimeKind::Bun => {
                let mut args = vec![
                    "--bun".to_owned(),
                    "run".to_owned(),
                    "--no-addons".to_owned(),
                ];
                args.push(if options.allow_network {
                    "--prefer-offline".to_owned()
                } else {
                    "--no-install".to_owned()
                });
                args.push("-".to_owned());
                args
            }
            RuntimeKind::QuickJs => vec!["--script".to_owned(), String::new()],
        };
        RuntimeInvocation {
            program: self.info.path.clone(),
            args: std::mem::take(&mut args),
        }
    }

    pub fn execute(
        &self,
        script: &str,
        options: &RuntimeOptions,
    ) -> Result<JavascriptOutput, JavascriptError> {
        let mut invocation = self.invocation(options);
        let temporary = if self.info.kind == RuntimeKind::QuickJs {
            let path = unique_script_path();
            let mut file = File::create(&path)?;
            file.write_all(script.as_bytes())?;
            file.sync_all()?;
            if let Some(argument) = invocation.args.get_mut(1) {
                *argument = path.to_string_lossy().into_owned();
            }
            Some(path)
        } else {
            None
        };

        let mut command = Command::new(&invocation.program);
        command.args(&invocation.args);
        command.envs(&options.environment);
        command.stdin(if temporary.is_some() {
            Stdio::null()
        } else {
            Stdio::piped()
        });
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn()?;
        if temporary.is_none() {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(script.as_bytes())?;
            }
        }
        let output = child.wait_with_output()?;
        if let Some(path) = temporary {
            let _ = fs::remove_file(path);
        }
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() {
            return Err(JavascriptError::Failed {
                runtime: self.info.clone(),
                status: output.status.code(),
                stderr,
            });
        }
        Ok(JavascriptOutput {
            runtime: self.info.clone(),
            invocation,
            stdout,
            stderr,
            status: output.status.code().unwrap_or(0),
        })
    }

    pub fn execute_json(
        &self,
        script: &str,
        options: &RuntimeOptions,
    ) -> Result<Value, JavascriptError> {
        let output = self.execute(script, options)?;
        parse_json_output(&output.stdout)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_runtime_specific_invocations() {
        let info = RuntimeInfo {
            kind: RuntimeKind::Deno,
            name: "deno".to_owned(),
            path: PathBuf::from("deno"),
            version: "2.3.0".to_owned(),
            version_tuple: vec![2, 3, 0],
            supported: true,
        };
        let runtime = JavascriptRuntime::from_info(info).unwrap();
        let invocation = runtime.invocation(&RuntimeOptions::default());
        assert_eq!(invocation.args.last().map(String::as_str), Some("-"));
        assert!(invocation.args.iter().any(|arg| arg == "--no-remote"));
    }

    #[test]
    fn ejs_wrapper_is_data_driven_and_json_round_trips() {
        let script = build_ejs_script(
            "const lib = {};",
            "function jsc(value) { return value; }",
            &serde_json::json!({"type": "test"}),
        );
        assert!(script.contains("Object.assign(globalThis, lib);"));
        assert!(script.contains(r#"jsc({"type":"test"})"#));
        assert_eq!(
            parse_json_output("noise\n{\"ok\":true}\n").unwrap()["ok"],
            true
        );
    }

    #[test]
    fn probes_and_executes_node_when_available() {
        let Some(runtime) = JavascriptRuntime::probe(RuntimeKind::Node, None).unwrap() else {
            return;
        };
        if !runtime.info().supported {
            return;
        }
        let output = runtime
            .execute("console.log(2 + 2)", &RuntimeOptions::default())
            .unwrap();
        assert_eq!(output.stdout.trim(), "4");
    }
}
