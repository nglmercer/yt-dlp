//! Explicit Python compatibility backend for the incremental Rust migration.
//!
//! This crate is intentionally process-based. It gives the Rust executable a
//! stable escape hatch for extractors, plugins, JavaScript, and option groups
//! that are not native yet, while keeping all argument boundaries free of a
//! shell and making the hand-off testable.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::process::{Command, ExitStatus, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonBackend {
    executable: OsString,
    module: OsString,
}

impl Default for PythonBackend {
    fn default() -> Self {
        Self::from_environment()
    }
}

impl PythonBackend {
    pub fn new(executable: impl Into<OsString>) -> Self {
        Self {
            executable: executable.into(),
            module: OsString::from("yt_dlp"),
        }
    }

    pub fn from_environment() -> Self {
        Self::new(std::env::var_os("YT_DLP_PYTHON").unwrap_or_else(|| OsString::from("python3")))
    }

    pub fn with_module(mut self, module: impl Into<OsString>) -> Self {
        self.module = module.into();
        self
    }

    pub fn executable(&self) -> &OsStr {
        &self.executable
    }

    pub fn command(&self, args: &[String]) -> Command {
        let mut command = Command::new(&self.executable);
        command.arg("-m").arg(&self.module).args(args);
        command
    }

    pub fn run_inherit(&self, args: &[String]) -> Result<ExitStatus, CompatibilityError> {
        self.command(args).status().map_err(CompatibilityError::Io)
    }

    pub fn run_capture(&self, args: &[String]) -> Result<CompatibilityOutput, CompatibilityError> {
        let output = self
            .command(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(CompatibilityError::Io)?;
        Ok(CompatibilityOutput {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    pub fn dump_single_json(
        &self,
        url: &str,
        additional_args: &[String],
    ) -> Result<serde_json::Value, CompatibilityError> {
        let mut args = vec![
            "--dump-single-json".to_owned(),
            "--skip-download".to_owned(),
        ];
        args.extend(additional_args.iter().cloned());
        args.push(url.to_owned());
        let output = self.run_capture(&args)?;
        if !output.status.success() {
            return Err(CompatibilityError::Failed {
                status: output.status,
                stderr: output.stderr,
            });
        }
        serde_json::from_str(&output.stdout).map_err(CompatibilityError::Json)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
pub enum CompatibilityError {
    Io(std::io::Error),
    Failed { status: ExitStatus, stderr: String },
    Json(serde_json::Error),
}

impl fmt::Display for CompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Failed { status, stderr } => {
                write!(
                    formatter,
                    "Python compatibility backend exited with {status}: {}",
                    stderr.trim()
                )
            }
            Self::Json(error) => write!(
                formatter,
                "Python compatibility backend returned invalid JSON: {error}"
            ),
        }
    }
}

impl std::error::Error for CompatibilityError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_an_argument_vector_without_shell_interpolation() {
        let backend = PythonBackend::new("/usr/bin/python3");
        let command = backend.command(&[
            "--output".to_owned(),
            "$(touch should-not-run).mp4".to_owned(),
            "https://example.test/?q=a b".to_owned(),
        ]);
        let args = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(args[0..2], ["-m", "yt_dlp"]);
        assert!(args.contains(&"$(touch should-not-run).mp4".to_owned()));
        assert!(args.contains(&"https://example.test/?q=a b".to_owned()));
    }

    #[test]
    fn capture_backend_can_be_tested_with_a_non_python_module() {
        let backend = PythonBackend::new("/bin/echo").with_module("yt_dlp");
        let output = backend.run_capture(&["--version".to_owned()]).unwrap();
        assert!(output.status.success());
        assert!(output.stdout.contains("-m yt_dlp --version"));
    }
}
