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
    /// Extra flags inserted before the script/stdin argument, e.g. Node
    /// permission flags for the challenge-solver invocation.
    pub extra_args: Vec<String>,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            allow_network: false,
            no_check_certificate: false,
            jitless: false,
            environment: BTreeMap::new(),
            extra_args: Vec::new(),
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
