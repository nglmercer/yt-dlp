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
