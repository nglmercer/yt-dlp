fn alias_arity(template: &str) -> usize {
    let mut arity = 0;
    let bytes = template.as_bytes();
    for index in 0..bytes.len().saturating_sub(2) {
        if bytes[index] == b'{' && bytes[index + 1].is_ascii_digit() && bytes[index + 2] == b'}' {
            arity = arity.max((bytes[index + 1] - b'0' + 1) as usize);
        }
    }
    arity
}

fn expand_alias_token(template: &str, values: &[String]) -> String {
    let mut expanded = template.to_owned();
    for (index, value) in values.iter().enumerate() {
        expanded = expanded.replace(&format!("{{{index}}}"), value);
    }
    expanded
}

fn expand_aliases(args: &[String]) -> Result<Vec<String>, CliError> {
    let mut aliases = IndexMap::new();
    let mut definitions_removed = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--alias" => {
                let names = args
                    .get(index + 1)
                    .ok_or_else(|| missing_value("--alias"))?;
                let template = args
                    .get(index + 2)
                    .ok_or_else(|| missing_value("--alias"))?;
                for name in names
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                {
                    let name = if name.starts_with('-') {
                        name.to_owned()
                    } else {
                        format!("--{name}")
                    };
                    aliases.insert(name, template.clone());
                }
                index += 3;
            }
            "--preset-alias" | "-t" => {
                let preset = args
                    .get(index + 1)
                    .ok_or_else(|| missing_value(args[index].as_str()))?;
                let template = match preset.as_str() {
                    "mp3" => "-f ba[acodec^=mp3]/ba/b -x --audio-format mp3",
                    "aac" => "-f ba[acodec^=aac]/ba[acodec^=mp4a.40.]/ba/b -x --audio-format aac",
                    "mp4" => {
                        "--merge-output-format mp4 --remux-video mp4 -S vcodec:h264,lang,quality,res,fps,hdr:12,acodec:aac"
                    }
                    "mkv" => "--merge-output-format mkv --remux-video mkv",
                    "sleep" => {
                        "--sleep-subtitles 5 --sleep-requests 0.75 --sleep-interval 10 --max-sleep-interval 20"
                    }
                    _ => return Err(CliError::new(format!("unknown preset alias: {preset}"))),
                };
                definitions_removed.extend(split_shell_words(template)?);
                index += 2;
            }
            _ => {
                definitions_removed.push(args[index].clone());
                index += 1;
            }
        }
    }

    let mut expanded = definitions_removed;
    for _ in 0..100 {
        let mut changed = false;
        let mut next = Vec::with_capacity(expanded.len());
        let mut index = 0;
        while index < expanded.len() {
            let token = &expanded[index];
            let Some(template) = aliases.get(token) else {
                next.push(token.clone());
                index += 1;
                continue;
            };
            let arity = alias_arity(template);
            if expanded.len() < index + 1 + arity {
                return Err(CliError::new(format!(
                    "{token} requires {arity} argument{}",
                    if arity == 1 { "" } else { "s" }
                )));
            }
            let values = &expanded[index + 1..index + 1 + arity];
            let replacement = split_shell_words(&expand_alias_token(template, values))?;
            next.extend(replacement);
            index += 1 + arity;
            changed = true;
        }
        expanded = next;
        if !changed {
            return Ok(expanded);
        }
    }
    Err(CliError::new("alias exceeded invocation limit"))
}
