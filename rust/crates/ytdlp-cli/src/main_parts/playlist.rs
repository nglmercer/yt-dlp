fn native_input_urls(options: &cli::CliOptions) -> Result<Vec<String>, String> {
    let mut urls = options.urls.clone();
    if let Some(batchfile) = options.batchfile.as_deref() {
        let contents = if batchfile == "-" {
            let mut contents = String::new();
            io::stdin()
                .read_to_string(&mut contents)
                .map_err(|error| format!("could not read batch file from stdin: {error}"))?;
            contents
        } else {
            std::fs::read_to_string(batchfile)
                .map_err(|error| format!("could not read batch file {batchfile:?}: {error}"))?
        };
        urls.extend(
            contents
                .lines()
                .map(str::trim)
                .filter(|url| !url.is_empty() && !url.starts_with('#'))
                .map(str::to_owned),
        );
    }
    if urls.is_empty() {
        return Err("at least one URL or --batch-file entry is required".to_owned());
    }
    Ok(urls)
}

fn native_playlist_indices(spec: Option<&str>, length: usize) -> Result<Vec<usize>, String> {
    if length == 0 {
        return Ok(Vec::new());
    }
    let Some(spec) = spec else {
        return Ok((0..length).collect());
    };
    let mut indices = Vec::new();
    for token in spec.split(',').map(str::trim) {
        if token.is_empty() {
            return Err("--playlist-items cannot contain an empty segment".to_owned());
        }
        let range = token.find(':').or_else(|| {
            token
                .as_bytes()
                .iter()
                .enumerate()
                .skip(1)
                .find_map(|(position, value)| (*value == b'-').then_some(position))
        });
        let Some(separator) = range else {
            let value = token
                .parse::<i64>()
                .map_err(|error| format!("invalid --playlist-items value {token:?}: {error}"))?;
            let index = if value >= 0 {
                value - 1
            } else {
                length as i64 + value
            };
            if (0..length as i64).contains(&index) {
                indices.push(index as usize);
            }
            continue;
        };

        let (start_text, remainder) = token.split_at(separator);
        let remainder = &remainder[1..];
        let (end_text, step_text) = remainder.split_once(':').unwrap_or((remainder, ""));
        let start =
            if start_text.is_empty() {
                None
            } else {
                Some(start_text.parse::<i64>().map_err(|error| {
                    format!("invalid --playlist-items range {token:?}: {error}")
                })?)
            };
        let end =
            if end_text.is_empty()
                || end_text.eq_ignore_ascii_case("inf")
                || end_text.eq_ignore_ascii_case("infinite")
            {
                None
            } else {
                Some(end_text.parse::<i64>().map_err(|error| {
                    format!("invalid --playlist-items range {token:?}: {error}")
                })?)
            };
        let step = if step_text.is_empty() {
            1
        } else {
            step_text
                .parse::<i64>()
                .map_err(|error| format!("invalid --playlist-items step {token:?}: {error}"))?
        };
        if step == 0 {
            return Err(format!(
                "step in --playlist-items segment {token:?} cannot be zero"
            ));
        }

        let start_index = match start {
            Some(value) if value >= 0 => value - 1,
            Some(value) => length as i64 + value,
            None if step > 0 => 0,
            None => length as i64 - 1,
        };
        let stop_index = match end {
            Some(value) if value >= 0 => value - 1,
            Some(value) => length as i64 + value,
            None if step > 0 => length as i64,
            None => -1,
        } + if step > 0 { 1 } else { -1 };

        let mut index = start_index;
        while if step > 0 {
            index < stop_index
        } else {
            index > stop_index
        } {
            if (0..length as i64).contains(&index) {
                indices.push(index as usize);
            }
            index = match index.checked_add(step) {
                Some(index) => index,
                None => break,
            };
        }
    }
    Ok(indices)
}
