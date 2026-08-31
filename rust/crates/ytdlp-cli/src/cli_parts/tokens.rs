fn missing_value(option: &str) -> CliError {
    CliError::new(format!("{option} requires an argument"))
}

fn next_value(args: &[String], index: &mut usize, option: &str) -> Result<String, CliError> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| missing_value(option))
}

fn option_value(
    args: &[String],
    index: &mut usize,
    option: &str,
    inline_value: Option<String>,
) -> Result<String, CliError> {
    inline_value.map_or_else(|| next_value(args, index, option), Ok)
}

fn parse_f64(value: String, option: &str) -> Result<f64, CliError> {
    value
        .parse()
        .map_err(|error| CliError::new(format!("invalid value for {option}: {error}")))
}

fn parse_i64(value: String, option: &str) -> Result<i64, CliError> {
    value
        .parse()
        .map_err(|error| CliError::new(format!("invalid value for {option}: {error}")))
}

fn split_long_option(argument: &str) -> (&str, Option<String>) {
    argument
        .split_once('=')
        .map_or((argument, None), |(name, value)| {
            (name, Some(value.to_owned()))
        })
}

fn add_csv(values: &mut Vec<String>, value: &str, prepend: bool) {
    let parsed = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if prepend {
        values.splice(0..0, parsed);
    } else {
        values.extend(parsed);
    }
}

fn add_prefixed_value(
    values: &mut IndexMap<String, String>,
    value: &str,
    allowed_prefixes: &[&str],
    default_prefix: &str,
) {
    if let Some((prefix, value)) = value.split_once(':') {
        if allowed_prefixes.contains(&prefix) {
            values.insert(prefix.to_owned(), value.to_owned());
            return;
        }
    }
    values.insert(default_prefix.to_owned(), value.to_owned());
}

fn add_postprocessor_args(
    values: &mut IndexMap<String, Vec<String>>,
    value: &str,
) -> Result<(), CliError> {
    let (key, arguments) = value
        .split_once(':')
        .ok_or_else(|| CliError::new("--postprocessor-args must be NAME:ARGS"))?;
    if key.is_empty() {
        return Err(CliError::new(
            "--postprocessor-args requires a non-empty processor name",
        ));
    }
    let key = key.to_ascii_lowercase();
    values
        .entry(key)
        .or_default()
        .extend(split_shell_words(arguments)?);
    Ok(())
}

fn split_shell_words(value: &str) -> Result<Vec<String>, CliError> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut in_word = false;

    for character in value.chars() {
        if escaped {
            word.push(character);
            escaped = false;
            in_word = true;
            continue;
        }
        match quote {
            Some('\'') if character != '\'' => word.push(character),
            Some('\'') => quote = None,
            Some('"') if character == '"' => quote = None,
            Some('"') if character == '\\' => escaped = true,
            Some('"') => word.push(character),
            Some(_) => word.push(character),
            None if character == '\\' => escaped = true,
            None if character == '\'' || character == '"' => {
                quote = Some(character);
                in_word = true;
            }
            None if character.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(character);
                in_word = true;
            }
        }
    }
    if escaped || quote.is_some() {
        return Err(CliError::new("unterminated quote or escape in alias"));
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

fn strip_config_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some('"') if character == '\\' => escaped = true,
            Some('"') if character == '"' => quote = None,
            Some('\'') if character == '\'' => quote = None,
            Some(_) => {}
            None if character == '\'' || character == '"' => quote = Some(character),
            None if character == '#' => return &line[..index],
            None => {}
        }
    }
    line
}

/// Read a yt-dlp configuration file into the same argument tokens used by
/// the command line. Missing-file policy is left to the caller so explicit
/// config locations can fail loudly while default locations can be skipped.
pub fn read_config_file(path: &Path) -> Result<Vec<String>, CliError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| CliError::new(format!("could not read config {path:?}: {error}")))?;
    contents
        .lines()
        .try_fold(Vec::new(), |mut arguments, line| {
            arguments.extend(split_shell_words(strip_config_comment(line))?);
            Ok(arguments)
        })
}
