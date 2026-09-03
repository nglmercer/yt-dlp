impl JavascriptRuntime {
    /// Probe one configured runtime. An unavailable executable returns
    /// `Ok(None)`, matching yt-dlp's ability to try the next provider.
    pub fn probe(
        kind: RuntimeKind,
        location: Option<&Path>,
    ) -> Result<Option<Self>, JavascriptError> {
        let path = determine_runtime_path(kind, location);
        let args = if kind == RuntimeKind::QuickJs {
            vec!["--help"]
        } else {
            vec!["--version"]
        };
        let output = match Command::new(&path).args(&args).output() {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let version = parse_runtime_version(kind, &combined).ok_or_else(|| {
            JavascriptError::InvalidVersion {
                kind,
                output: combined.clone(),
            }
        })?;
        let version_tuple = parse_version_tuple(&version);
        let info = RuntimeInfo {
            kind,
            name: if kind == RuntimeKind::QuickJs && combined.contains("QuickJS-ng") {
                "quickjs-ng".to_owned()
            } else {
                kind.name().to_owned()
            },
            path,
            version,
            supported: version_tuple.as_slice() >= kind.minimum_supported(),
            version_tuple,
        };
        Ok(Some(Self { info }))
    }

    pub fn from_info(info: RuntimeInfo) -> Result<Self, JavascriptError> {
        if !info.supported {
            return Err(JavascriptError::Unsupported(info));
        }
        Ok(Self { info })
    }

    pub fn info(&self) -> &RuntimeInfo {
        &self.info
    }

    pub fn invocation(&self, options: &RuntimeOptions) -> RuntimeInvocation {
        let mut args = self.base_invocation(options);
        // Extra flags go before the final stdin/script argument.
        if !options.extra_args.is_empty() {
            if let Some(tail) = args.pop() {
                args.extend(options.extra_args.iter().cloned());
                args.push(tail);
            } else {
                args.extend(options.extra_args.iter().cloned());
            }
        }
        RuntimeInvocation {
            program: self.info.path.clone(),
            args: std::mem::take(&mut args),
        }
    }

    fn base_invocation(&self, options: &RuntimeOptions) -> Vec<String> {
        match self.info.kind {
            RuntimeKind::Deno => {
                let mut args = vec![
                    "run".to_owned(),
                    "--ext=js".to_owned(),
                    "--no-code-cache".to_owned(),
                    "--no-prompt".to_owned(),
                    "--no-lock".to_owned(),
                    "--node-modules-dir=none".to_owned(),
                    "--no-config".to_owned(),
                ];
                if !options.allow_network {
                    args.extend([
                        "--no-remote".to_owned(),
                        "--no-npm".to_owned(),
                        "--cached-only".to_owned(),
                    ]);
                }
                if options.no_check_certificate {
                    args.push("--unsafely-ignore-certificate-errors".to_owned());
                }
                if options.jitless {
                    args.push("--v8-flags=--jitless".to_owned());
                }
                args.push("-".to_owned());
                args
            }
            RuntimeKind::Node => {
                let mut args = Vec::new();
                if options.jitless {
                    args.push("--v8-flags=--jitless".to_owned());
                }
                args.push("-".to_owned());
                args
            }
            RuntimeKind::Bun => {
                let mut args = vec![
                    "--bun".to_owned(),
                    "run".to_owned(),
                    "--no-addons".to_owned(),
                ];
                args.push(if options.allow_network {
                    "--prefer-offline".to_owned()
                } else {
                    "--no-install".to_owned()
                });
                args.push("-".to_owned());
                args
            }
            RuntimeKind::QuickJs => vec!["--script".to_owned(), String::new()],
        }
    }

    pub fn execute(
        &self,
        script: &str,
        options: &RuntimeOptions,
    ) -> Result<JavascriptOutput, JavascriptError> {
        let mut invocation = self.invocation(options);
        let temporary = if self.info.kind == RuntimeKind::QuickJs {
            let path = unique_script_path();
            let mut file = File::create(&path)?;
            file.write_all(script.as_bytes())?;
            file.sync_all()?;
            if let Some(argument) = invocation.args.get_mut(1) {
                *argument = path.to_string_lossy().into_owned();
            }
            Some(path)
        } else {
            None
        };

        let mut command = Command::new(&invocation.program);
        command.args(&invocation.args);
        command.envs(&options.environment);
        command.stdin(if temporary.is_some() {
            Stdio::null()
        } else {
            Stdio::piped()
        });
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn()?;
        if temporary.is_none() {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(script.as_bytes())?;
            }
        }
        let output = child.wait_with_output()?;
        if let Some(path) = temporary {
            let _ = fs::remove_file(path);
        }
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() {
            return Err(JavascriptError::Failed {
                runtime: self.info.clone(),
                status: output.status.code(),
                stderr,
            });
        }
        Ok(JavascriptOutput {
            runtime: self.info.clone(),
            invocation,
            stdout,
            stderr,
            status: output.status.code().unwrap_or(0),
        })
    }

    pub fn execute_json(
        &self,
        script: &str,
        options: &RuntimeOptions,
    ) -> Result<Value, JavascriptError> {
        let output = self.execute(script, options)?;
        parse_json_output(&output.stdout)
    }
}
