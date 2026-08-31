#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOptions {
    pub simulate: bool,
    pub overwrite: bool,
    pub resume: bool,
    pub retries: usize,
    pub concurrent: usize,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            simulate: false,
            overwrite: true,
            resume: true,
            retries: 10,
            concurrent: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResult {
    pub url: String,
    pub status: u16,
    pub bytes: usize,
    pub path: Option<PathBuf>,
    pub simulated: bool,
    pub fragments: Option<usize>,
    pub resumed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub index: usize,
    pub request: Request,
}

#[derive(Debug)]
pub enum DownloadError {
    Request(RequestError),
    Io(io::Error),
    OutputExists(PathBuf),
    InvalidOutput(PathBuf),
    InvalidPlaylist(String),
    Unsupported(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::OutputExists(path) => write!(formatter, "output already exists: {path:?}"),
            Self::InvalidOutput(path) => write!(formatter, "invalid output path: {path:?}"),
            Self::InvalidPlaylist(message) => write!(formatter, "invalid HLS playlist: {message}"),
            Self::Unsupported(message) => write!(formatter, "unsupported download: {message}"),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<RequestError> for DownloadError {
    fn from(error: RequestError) -> Self {
        Self::Request(error)
    }
}

impl From<io::Error> for DownloadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
