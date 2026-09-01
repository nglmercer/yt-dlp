pub struct DirectDownloader {
    director: RequestDirector,
}

impl DirectDownloader {
    pub fn new(director: RequestDirector) -> Self {
        Self { director }
    }

    pub fn native() -> Self {
        Self::new(RequestDirector::native())
    }

    fn send_with_retries(
        &self,
        request: &Request,
        retries: usize,
    ) -> Result<yt_dlp_networking::Response, DownloadError> {
        let mut last_error = None;
        for attempt in 0..=retries {
            match self.director.send(request) {
                Ok(response) if response.status() >= 500 && attempt < retries => continue,
                Ok(response) => return Ok(response),
                Err(error) if attempt < retries => last_error = Some(error),
                Err(error) => return Err(error.into()),
            }
        }
        Err(DownloadError::Request(last_error.unwrap_or_else(|| {
            RequestError::new(ErrorKind::Transport, "download retry failed")
        })))
    }

    fn check_response(response: &yt_dlp_networking::Response) -> Result<(), DownloadError> {
        if response.status() >= 400 {
            return Err(DownloadError::Request(RequestError::new(
                ErrorKind::Http {
                    status: response.status(),
                    reason: response.reason().to_owned(),
                },
                format!("HTTP request failed with status {}", response.status()),
            )));
        }
        Ok(())
    }

    pub fn download(
        &self,
        request: &Request,
        output: Option<&Path>,
        options: &DownloadOptions,
    ) -> Result<DownloadResult, DownloadError> {
        let mut request = request.clone();
        let mut prefix = Vec::new();
        let mut resumed = false;
        if options.resume {
            if let Some(output) = output {
                if output.is_file() {
                    prefix = fs::read(output)?;
                    if !prefix.is_empty() {
                        request
                            .headers_mut()
                            .set("Range", format!("bytes={}-", prefix.len()));
                    }
                }
            }
        }
        let response = self.send_with_retries(&request, options.retries)?;
        Self::check_response(&response)?;
        let mut body = response.body().to_vec();
        if !prefix.is_empty() && response.status() == 206 {
            prefix.extend_from_slice(&body);
            body = prefix;
            resumed = true;
        }

        let path = if options.simulate {
            None
        } else if let Some(output) = output {
            Some(write_atomic(output, &body, options.overwrite || resumed)?)
        } else {
            None
        };

        Ok(DownloadResult {
            url: response.url().to_owned(),
            status: response.status(),
            bytes: body.len(),
            path,
            simulated: options.simulate,
            fragments: None,
            resumed,
        })
    }

    /// Download an HLS media playlist and concatenate its initialization and
    /// media segments in playlist order. Master playlists select their first
    /// variant until adaptive selection is added.
    pub fn download_hls(
        &self,
        request: &Request,
        output: Option<&Path>,
        options: &DownloadOptions,
    ) -> Result<DownloadResult, DownloadError> {
        let manifest = self.send_with_retries(request, options.retries)?;
        Self::check_response(&manifest)?;
        let playlist = parse_hls_playlist(request.url(), manifest.body())?;
        if let Some(variant) = playlist.variant {
            let mut variant_request = request.clone();
            variant_request.set_url(variant);
            variant_request.set_data(None);
            variant_request.set_method("GET")?;
            return self.download_hls(&variant_request, output, options);
        }

        let fragments = playlist
            .segments
            .iter()
            .enumerate()
            .zip(playlist.segment_ranges.iter())
            .map(|((index, segment), range)| {
                let mut segment_request = request.clone();
                segment_request.set_url(segment);
                segment_request.set_data(None);
                segment_request.set_method("GET")?;
                native_apply_extra_param_to_segment_url(&mut segment_request)?;
                if let Some(range) = range {
                    segment_request.headers_mut().set(
                        "Range",
                        format!("bytes={}-{}", range.start, range.end_inclusive()?),
                    );
                }
                Ok(Fragment {
                    index,
                    request: segment_request,
                })
            })
            .collect::<Result<Vec<_>, DownloadError>>()?;
        let (status, body) = self.fetch_fragments(&fragments, options)?;
        let path = if options.simulate {
            None
        } else if let Some(output) = output {
            Some(write_atomic(output, &body, options.overwrite)?)
        } else {
            None
        };
        Ok(DownloadResult {
            url: request.url().to_owned(),
            status,
            bytes: body.len(),
            path,
            simulated: options.simulate,
            fragments: Some(fragments.len()),
            resumed: false,
        })
    }

    fn fetch_fragments(
        &self,
        fragments: &[Fragment],
        options: &DownloadOptions,
    ) -> Result<(u16, Vec<u8>), DownloadError> {
        if fragments.is_empty() {
            return Err(DownloadError::InvalidPlaylist(
                "no media fragments".to_owned(),
            ));
        }
        let worker_count = options.concurrent.max(1).min(fragments.len());
        let queue = Arc::new(Mutex::new(VecDeque::from(fragments.to_vec())));
        let results = Arc::new(Mutex::new(Vec::with_capacity(fragments.len())));
        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let queue = Arc::clone(&queue);
                let results = Arc::clone(&results);
                scope.spawn(move || {
                    loop {
                        let fragment = queue.lock().ok().and_then(|mut queue| queue.pop_front());
                        let Some(fragment) = fragment else {
                            break;
                        };
                        let result = self
                            .send_with_retries(&fragment.request, options.retries)
                            .and_then(|response| {
                                Self::check_response(&response)?;
                                Ok((fragment.index, response.status(), response.body().to_vec()))
                            });
                        let failed = result.is_err();
                        if let Ok(mut results) = results.lock() {
                            results.push(result);
                        }
                        if failed {
                            break;
                        }
                    }
                });
            }
        });

        let mut results = Arc::try_unwrap(results)
            .map_err(|_| DownloadError::InvalidPlaylist("fragment result lock busy".to_owned()))?
            .into_inner()
            .map_err(|_| {
                DownloadError::InvalidPlaylist("fragment result lock poisoned".to_owned())
            })?;
        if let Some(position) = results.iter().position(Result::is_err) {
            if let Err(error) = results.swap_remove(position) {
                return Err(error);
            }
        }
        results.sort_by_key(|result| result.as_ref().map_or(usize::MAX, |result| result.0));
        let status = results
            .first()
            .and_then(|result| result.as_ref().ok().map(|result| result.1))
            .unwrap_or(200);
        let mut body = Vec::new();
        for result in results {
            let (_, _, fragment_body) = result?;
            body.extend_from_slice(&fragment_body);
        }
        Ok((status, body))
    }

    /// Fetch an explicitly ordered fragment set and atomically assemble it.
    pub fn download_fragments(
        &self,
        fragments: &[Fragment],
        output: Option<&Path>,
        options: &DownloadOptions,
    ) -> Result<DownloadResult, DownloadError> {
        let (status, body) = self.fetch_fragments(fragments, options)?;
        let path = if options.simulate {
            None
        } else if let Some(output) = output {
            Some(write_atomic(output, &body, options.overwrite)?)
        } else {
            None
        };
        Ok(DownloadResult {
            url: fragments
                .first()
                .map(|fragment| fragment.request.url().to_owned())
                .unwrap_or_default(),
            status,
            bytes: body.len(),
            path,
            simulated: options.simulate,
            fragments: Some(fragments.len()),
            resumed: false,
        })
    }

    pub fn download_dash(
        &self,
        request: &Request,
        output: Option<&Path>,
        options: &DownloadOptions,
    ) -> Result<DownloadResult, DownloadError> {
        let manifest = self.send_with_retries(request, options.retries)?;
        Self::check_response(&manifest)?;
        let playlist = parse_dash_mpd(request.url(), manifest.body())?;
        let fragments = playlist
            .segments
            .iter()
            .enumerate()
            .zip(playlist.segment_ranges.iter())
            .map(|((index, segment), range)| {
                let mut segment_request = request.clone();
                segment_request.set_url(segment);
                segment_request.set_data(None);
                segment_request.set_method("GET")?;
                native_apply_extra_param_to_segment_url(&mut segment_request)?;
                if let Some(range) = range {
                    segment_request.headers_mut().set(
                        "Range",
                        format!("bytes={}-{}", range.start, range.end_inclusive()?),
                    );
                }
                Ok(Fragment {
                    index,
                    request: segment_request,
                })
            })
            .collect::<Result<Vec<_>, DownloadError>>()?;
        let (status, body) = self.fetch_fragments(&fragments, options)?;
        let path = if options.simulate {
            None
        } else if let Some(output) = output {
            Some(write_atomic(output, &body, options.overwrite)?)
        } else {
            None
        };
        Ok(DownloadResult {
            url: request.url().to_owned(),
            status: manifest.status().max(status),
            bytes: body.len(),
            path,
            simulated: options.simulate,
            fragments: Some(fragments.len()),
            resumed: false,
        })
    }
}
