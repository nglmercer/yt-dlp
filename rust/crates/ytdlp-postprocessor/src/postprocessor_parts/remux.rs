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
