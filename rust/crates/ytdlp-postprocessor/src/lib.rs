//! Postprocessing contracts for the Rust migration.
//!
//! yt-dlp's postprocessors are stateful Python classes, but their observable
//! contract is small: receive an info dictionary containing `filepath`, run a
//! tool when needed, return the updated info dictionary, and identify files
//! that may be removed after a successful operation.  This crate establishes
//! that contract and provides the first safe FFmpeg subprocess integration.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use indexmap::IndexMap;
use serde_json::json;
use yt_dlp_core::InfoDict;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostProcessOptions {
    pub ffmpeg_location: Option<PathBuf>,
    pub overwrite: bool,
    pub keep_video: bool,
    pub simulate: bool,
    pub extra_args: IndexMap<String, Vec<String>>,
}

impl Default for PostProcessOptions {
    fn default() -> Self {
        Self {
            ffmpeg_location: None,
            overwrite: true,
            keep_video: false,
            simulate: false,
            extra_args: IndexMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostProcessResult {
    pub files_to_delete: Vec<PathBuf>,
    pub info: InfoDict,
    pub command: Option<Vec<OsString>>,
    pub simulated: bool,
}

#[derive(Debug)]
pub enum PostProcessError {
    MissingField(String),
    InvalidPath(String),
    OutputExists(PathBuf),
    MissingOutput(PathBuf),
    Unsupported(String),
    Io(std::io::Error),
    Failed {
        program: PathBuf,
        status: Option<i32>,
        stderr: String,
    },
}

impl fmt::Display for PostProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => {
                write!(formatter, "postprocessor field is missing: {field}")
            }
            Self::InvalidPath(message) => {
                write!(formatter, "invalid postprocessor path: {message}")
            }
            Self::OutputExists(path) => {
                write!(formatter, "postprocessed output already exists: {path:?}")
            }
            Self::MissingOutput(path) => {
                write!(formatter, "FFmpeg did not create output: {path:?}")
            }
            Self::Unsupported(message) => {
                write!(formatter, "unsupported postprocessing: {message}")
            }
            Self::Io(error) => error.fmt(formatter),
            Self::Failed {
                program,
                status,
                stderr,
            } => write!(
                formatter,
                "{} failed with status {:?}: {}",
                program.display(),
                status,
                stderr.trim()
            ),
        }
    }
}

impl std::error::Error for PostProcessError {}

impl From<std::io::Error> for PostProcessError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// The common postprocessor lifecycle. Implementations must only report files
/// for deletion after their operation succeeds.
pub trait PostProcessor: Send + Sync {
    fn key(&self) -> &str;

    fn run(
        &self,
        info: &InfoDict,
        options: &PostProcessOptions,
    ) -> Result<PostProcessResult, PostProcessError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopPostProcessor;

impl PostProcessor for NoopPostProcessor {
    fn key(&self) -> &str {
        "Noop"
    }

    fn run(
        &self,
        info: &InfoDict,
        _options: &PostProcessOptions,
    ) -> Result<PostProcessResult, PostProcessError> {
        Ok(PostProcessResult {
            files_to_delete: Vec::new(),
            info: info.clone(),
            command: None,
            simulated: false,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegCommand {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl FfmpegCommand {
    pub fn argv(&self) -> Vec<OsString> {
        std::iter::once(self.program.as_os_str().to_os_string())
            .chain(self.args.iter().cloned())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct FfmpegRunner {
    executable: PathBuf,
}

impl FfmpegRunner {
    pub fn new(location: Option<&Path>) -> Self {
        Self {
            executable: resolve_ffmpeg(location),
        }
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn build_command(
        &self,
        input: &Path,
        output: &Path,
        operation_args: &[String],
        options: &PostProcessOptions,
        processor_key: &str,
    ) -> FfmpegCommand {
        let mut args = vec![
            if options.overwrite { "-y" } else { "-n" }.to_owned(),
            "-loglevel".to_owned(),
            "repeat+info".to_owned(),
        ];
        args.extend(extra_args(options, processor_key, "ffmpeg_i1"));
        args.push("-i".to_owned());
        args.push(ffmpeg_file_argument(input));
        args.extend(operation_args.iter().cloned());
        args.extend(extra_args(options, processor_key, "ffmpeg_o1"));
        args.push("-movflags".to_owned());
        args.push("+faststart".to_owned());
        args.push(ffmpeg_file_argument(output));
        FfmpegCommand {
            program: self.executable.clone(),
            args: args.into_iter().map(OsString::from).collect(),
        }
    }

    pub fn run(
        &self,
        command: &FfmpegCommand,
        simulate: bool,
    ) -> Result<Option<Vec<OsString>>, PostProcessError> {
        if simulate {
            return Ok(Some(command.argv()));
        }
        let output = Command::new(&command.program)
            .args(&command.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(PostProcessError::Io)?;
        if !output.status.success() {
            return Err(PostProcessError::Failed {
                program: command.program.clone(),
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(Some(command.argv()))
    }
}

#[derive(Debug, Clone)]
pub struct FfmpegRemuxer {
    target_ext: String,
}

impl FfmpegRemuxer {
    pub fn new(target_ext: impl Into<String>) -> Result<Self, PostProcessError> {
        let target_ext = target_ext.into().to_ascii_lowercase();
        validate_extension(&target_ext)?;
        Ok(Self { target_ext })
    }

    pub fn target_ext(&self) -> &str {
        &self.target_ext
    }
}

impl PostProcessor for FfmpegRemuxer {
    fn key(&self) -> &str {
        "FFmpegVideoRemuxer"
    }

    fn run(
        &self,
        info: &InfoDict,
        options: &PostProcessOptions,
    ) -> Result<PostProcessResult, PostProcessError> {
        let input = info_path(info)?;
        let output = input.with_extension(&self.target_ext);
        if output == input {
            return Ok(PostProcessResult {
                files_to_delete: Vec::new(),
                info: info.clone(),
                command: None,
                simulated: options.simulate,
            });
        }
        ensure_output_available(&output, options.overwrite)?;
        let runner = FfmpegRunner::new(options.ffmpeg_location.as_deref());
        let command = runner.build_command(
            &input,
            &output,
            &["-c".to_owned(), "copy".to_owned()],
            options,
            self.key(),
        );
        let argv = runner.run(&command, options.simulate)?;
        ensure_output_created(&output, options.simulate)?;
        let mut updated = info.clone();
        updated.insert("filepath", json!(output.to_string_lossy()));
        updated.insert("ext", json!(self.target_ext));
        updated.insert("format", json!(self.target_ext));
        Ok(PostProcessResult {
            files_to_delete: (!options.keep_video).then_some(input).into_iter().collect(),
            info: updated,
            command: argv,
            simulated: options.simulate,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FfmpegExtractAudio {
    target_ext: String,
    codec: Option<String>,
}

impl FfmpegExtractAudio {
    pub fn new(
        target_ext: impl Into<String>,
        codec: Option<String>,
    ) -> Result<Self, PostProcessError> {
        let target_ext = target_ext.into().to_ascii_lowercase();
        validate_extension(&target_ext)?;
        Ok(Self { target_ext, codec })
    }
}

impl PostProcessor for FfmpegExtractAudio {
    fn key(&self) -> &str {
        "FFmpegExtractAudio"
    }

    fn run(
        &self,
        info: &InfoDict,
        options: &PostProcessOptions,
    ) -> Result<PostProcessResult, PostProcessError> {
        let input = info_path(info)?;
        let output = input.with_extension(&self.target_ext);
        if output == input {
            return Ok(PostProcessResult {
                files_to_delete: Vec::new(),
                info: info.clone(),
                command: None,
                simulated: options.simulate,
            });
        }
        ensure_output_available(&output, options.overwrite)?;
        let mut operation_args = vec!["-vn".to_owned()];
        if let Some(codec) = &self.codec {
            operation_args.extend(["-acodec".to_owned(), codec.clone()]);
        }
        let runner = FfmpegRunner::new(options.ffmpeg_location.as_deref());
        let command = runner.build_command(&input, &output, &operation_args, options, self.key());
        let argv = runner.run(&command, options.simulate)?;
        ensure_output_created(&output, options.simulate)?;
        let mut updated = info.clone();
        updated.insert("filepath", json!(output.to_string_lossy()));
        updated.insert("ext", json!(self.target_ext));
        Ok(PostProcessResult {
            files_to_delete: (!options.keep_video).then_some(input).into_iter().collect(),
            info: updated,
            command: argv,
            simulated: options.simulate,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FfmpegVideoConvertor {
    target_ext: String,
}

impl FfmpegVideoConvertor {
    pub fn new(target_ext: impl Into<String>) -> Result<Self, PostProcessError> {
        let target_ext = target_ext.into().to_ascii_lowercase();
        validate_extension(&target_ext)?;
        Ok(Self { target_ext })
    }
}

impl PostProcessor for FfmpegVideoConvertor {
    fn key(&self) -> &str {
        "FFmpegVideoConvertor"
    }

    fn run(
        &self,
        info: &InfoDict,
        options: &PostProcessOptions,
    ) -> Result<PostProcessResult, PostProcessError> {
        let input = info_path(info)?;
        let output = input.with_extension(&self.target_ext);
        if output == input {
            return Ok(PostProcessResult {
                files_to_delete: Vec::new(),
                info: info.clone(),
                command: None,
                simulated: options.simulate,
            });
        }
        ensure_output_available(&output, options.overwrite)?;
        let (video_codec, audio_codec) = match self.target_ext.as_str() {
            "webm" => ("libvpx-vp9", "libopus"),
            "ogv" => ("libtheora", "libvorbis"),
            _ => ("libx264", "aac"),
        };
        let operation_args = vec![
            "-c:v".to_owned(),
            video_codec.to_owned(),
            "-c:a".to_owned(),
            audio_codec.to_owned(),
        ];
        let runner = FfmpegRunner::new(options.ffmpeg_location.as_deref());
        let command = runner.build_command(&input, &output, &operation_args, options, self.key());
        let argv = runner.run(&command, options.simulate)?;
        ensure_output_created(&output, options.simulate)?;
        let mut updated = info.clone();
        updated.insert("filepath", json!(output.to_string_lossy()));
        updated.insert("ext", json!(self.target_ext));
        updated.insert("format", json!(self.target_ext));
        Ok(PostProcessResult {
            files_to_delete: (!options.keep_video).then_some(input).into_iter().collect(),
            info: updated,
            command: argv,
            simulated: options.simulate,
        })
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_safe_remux_arguments_without_a_shell() {
        let runner = FfmpegRunner::new(Some(Path::new("/opt/tools/ffmpeg")));
        let mut options = PostProcessOptions::default();
        options
            .extra_args
            .insert("default-compat".to_owned(), vec!["-hide_banner".to_owned()]);
        let command = runner.build_command(
            Path::new("video;$(touch pwned).mp4"),
            Path::new("video.mkv"),
            &["-c".to_owned(), "copy".to_owned()],
            &options,
            "FFmpegVideoRemuxer",
        );
        assert_eq!(command.program, PathBuf::from("/opt/tools/ffmpeg"));
        assert!(
            command
                .args
                .iter()
                .any(|arg| arg == "file:video;$(touch pwned).mp4")
        );
        assert!(command.args.iter().any(|arg| arg == "-hide_banner"));
    }

    #[test]
    fn dry_run_updates_info_and_preserves_input_by_default_when_requested() {
        let mut info = InfoDict::new();
        info.insert("filepath", json!("video.webm"));
        info.insert("ext", json!("webm"));
        let mut options = PostProcessOptions {
            simulate: true,
            keep_video: true,
            ..PostProcessOptions::default()
        };
        options.ffmpeg_location = Some(PathBuf::from("ffmpeg"));
        let result = FfmpegRemuxer::new("mkv")
            .unwrap()
            .run(&info, &options)
            .unwrap();
        assert_eq!(result.info.get_str("filepath"), Some("video.mkv"));
        assert_eq!(result.info.get_str("ext"), Some("mkv"));
        assert!(result.files_to_delete.is_empty());
        assert!(result.simulated);
        assert!(result.command.is_some());
    }

    #[test]
    fn rejects_unsafe_extensions_and_missing_filepaths() {
        assert!(FfmpegRemuxer::new("../mkv").is_err());
        assert!(FfmpegExtractAudio::new("", None).is_err());
        assert!(matches!(
            FfmpegRemuxer::new("mkv")
                .unwrap()
                .run(&InfoDict::new(), &PostProcessOptions::default()),
            Err(PostProcessError::MissingField(_))
        ));
    }

    #[test]
    fn dry_run_converter_selects_target_codecs() {
        let mut info = InfoDict::new();
        info.insert("filepath", json!("video.mp4"));
        info.insert("ext", json!("mp4"));
        let result = FfmpegVideoConvertor::new("webm")
            .unwrap()
            .run(
                &info,
                &PostProcessOptions {
                    simulate: true,
                    ..PostProcessOptions::default()
                },
            )
            .unwrap();
        let command = result.command.unwrap();
        assert!(command.iter().any(|arg| arg == "libvpx-vp9"));
        assert!(command.iter().any(|arg| arg == "libopus"));
        assert_eq!(result.info.get_str("ext"), Some("webm"));
    }
}
