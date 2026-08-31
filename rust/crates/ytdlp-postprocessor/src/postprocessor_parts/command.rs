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
