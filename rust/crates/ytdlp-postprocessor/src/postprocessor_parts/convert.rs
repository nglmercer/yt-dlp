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
