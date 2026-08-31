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
