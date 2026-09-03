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

/// Stream probe resolver, mirroring the `get_audio_codec` half of
/// `FFmpegPostProcessor`: `ffprobe -show_streams` when a probe executable
/// resolves, otherwise the `ffmpeg -i` stderr fallback. Neither resolving
/// (an explicit ffmpeg file that is itself missing) surfaces the same
/// install hint Python raises.
#[derive(Debug, Clone)]
pub struct MediaProbe {
    ffprobe: Option<PathBuf>,
    ffmpeg: PathBuf,
}

impl MediaProbe {
    pub fn new(location: Option<&Path>) -> Self {
        Self {
            ffprobe: resolve_ffprobe(location),
            ffmpeg: resolve_ffmpeg(location),
        }
    }

    pub fn audio_codec(&self, path: &Path) -> Result<Option<String>, PostProcessError> {
        let argument = ffmpeg_file_argument(path);
        if let Some(ffprobe) = &self.ffprobe {
            let output = Command::new(ffprobe)
                .args(["-show_streams", &argument])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            // Mirrors `except OSError: return None` plus the nonzero-exit
            // early return: only a clean ffprobe run feeds the scanner.
            let Ok(output) = output else {
                return Ok(None);
            };
            if !output.status.success() {
                return Ok(None);
            }
            return Ok(scan_ffprobe_audio_codec(&String::from_utf8_lossy(
                &output.stdout,
            )));
        }
        let output = Command::new(&self.ffmpeg)
            .args(["-i", &argument])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    PostProcessError::Unsupported(
                        "ffprobe and ffmpeg not found. Please install or provide the path using --ffmpeg-location"
                            .to_owned(),
                    )
                } else {
                    PostProcessError::Io(error)
                }
            })?;
        // `ffmpeg -i` without an output always "fails"; code 1 is the
        // expected probe result, mirroring the `returncode != 1` check.
        if output.status.code() != Some(1) {
            return Ok(None);
        }
        Ok(scan_ffmpeg_audio_codec(&String::from_utf8_lossy(
            &output.stderr,
        )))
    }
}

/// Scan default (`-show_streams`) ffprobe output for the first audio stream's
/// codec, mirroring the `get_audio_codec` line loop exactly: the pending
/// `codec_name` is reported when a `codec_type=audio` line follows it (a
/// name placed after its type line is missed, just like upstream).
fn scan_ffprobe_audio_codec(output: &str) -> Option<String> {
    let mut audio_codec = None;
    for line in output.split('\n') {
        if let Some(codec) = line.strip_prefix("codec_name=") {
            audio_codec = Some(codec.trim().to_owned());
        } else if line.trim() == "codec_type=audio" && audio_codec.is_some() {
            return audio_codec;
        }
    }
    None
}

/// Parse the audio codec out of `ffmpeg -i` stderr, mirroring the fallback
/// `Stream\s*#\d+:\d+(?:\[0x[0-9a-f]+\])?(?:\([a-z]{3}\))?:\s*Audio:\s*([0-9a-z]+)`
/// search in `get_audio_codec`.
fn scan_ffmpeg_audio_codec(stderr: &str) -> Option<String> {
    let mut rest = stderr.to_owned();
    while let Some(position) = rest.find("Stream") {
        // Always advance past this occurrence so a failed parse keeps
        // scanning, mirroring regex `find` semantics.
        let mut fields = rest[position + "Stream".len()..].trim_start().to_owned();
        rest = rest[position + 1..].to_owned();
        fields = match parse_ffmpeg_stream_prefix(fields) {
            Some(fields) => fields,
            None => continue,
        };
        let Some(kind) = fields.strip_prefix("Audio:") else {
            continue;
        };
        let codec = kind
            .trim_start()
            .bytes()
            .take_while(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'z'))
            .collect::<Vec<_>>();
        if !codec.is_empty()
            && let Ok(codec) = String::from_utf8(codec)
        {
            return Some(codec);
        }
    }
    None
}

/// Consume `#FILE:STREAM`, an optional `[0xID]`, an optional `(LANG)`, and
/// the separating colon of one `ffmpeg -i` stream line, returning whatever
/// follows (expected to start with the codec type).
fn parse_ffmpeg_stream_prefix(mut fields: String) -> Option<String> {
    fields = fields.strip_prefix('#')?.to_owned();
    fields = fields
        .trim_start_matches(|character: char| character.is_ascii_digit())
        .to_owned();
    fields = fields.strip_prefix(':')?.to_owned();
    fields = fields
        .trim_start_matches(|character: char| character.is_ascii_digit())
        .to_owned();
    if let Some(hex) = fields.strip_prefix('[') {
        let hex = hex.strip_prefix("0x")?;
        let end = hex.find(|character: char| {
            !matches!(character, '0'..='9' | 'a'..='f')
        })?;
        if end == 0 || !hex[end..].starts_with(']') {
            return None;
        }
        fields = hex[end + 1..].to_owned();
    }
    if let Some(paren) = fields.strip_prefix('(') {
        let end = paren.find(')')?;
        if end != 3 || !paren[..end].bytes().all(|byte| byte.is_ascii_lowercase()) {
            return None;
        }
        fields = paren[end + 1..].to_owned();
    }
    Some(fields.strip_prefix(':')?.trim_start().to_owned())
}
