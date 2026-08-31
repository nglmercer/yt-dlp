fn render_template_value(value: &Value, format_spec: &str) -> Result<String, CoreError> {
    let conversion = format_spec.chars().last().ok_or_else(|| {
        CoreError::new(CoreErrorKind::InvalidInput, "empty output template format")
    })?;
    let modifiers = &format_spec[..format_spec.len() - conversion.len_utf8()];
    let zero_padded = modifiers.contains('0');
    let width = modifiers
        .trim_start_matches(['#', '0', '-', '+', ' '])
        .split_once('.')
        .map_or(
            modifiers.trim_start_matches(['#', '0', '-', '+', ' ']),
            |(width, _)| width,
        )
        .parse::<usize>()
        .unwrap_or(0);
    let precision = modifiers
        .split_once('.')
        .and_then(|(_, precision)| precision.parse::<usize>().ok());
    let rendered = match conversion {
        's' => match value {
            Value::String(value) => value.clone(),
            Value::Null => String::new(),
            value => value.to_string(),
        },
        'd' | 'i' => {
            let integer = value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
                .or_else(|| value.as_f64().map(|value| value as i64))
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
                .ok_or_else(|| {
                    CoreError::new(
                        CoreErrorKind::InvalidInput,
                        format!("value {value} is not an integer"),
                    )
                })?;
            integer.to_string()
        }
        'f' => {
            let number = value
                .as_f64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
                .ok_or_else(|| {
                    CoreError::new(
                        CoreErrorKind::InvalidInput,
                        format!("value {value} is not a number"),
                    )
                })?;
            precision.map_or_else(
                || number.to_string(),
                |precision| format!("{number:.precision$}"),
            )
        }
        _ => {
            return Err(CoreError::new(
                CoreErrorKind::InvalidInput,
                format!("unsupported output template format: {format_spec}"),
            ));
        }
    };
    if width <= rendered.len() {
        return Ok(rendered);
    }
    let padding = width - rendered.len();
    if zero_padded && conversion != 's' {
        Ok(format!("{}{}", "0".repeat(padding), rendered))
    } else {
        Ok(format!("{}{}", " ".repeat(padding), rendered))
    }
}

/// Render the initial Python-style output-template subset used by the native
/// downloader. Unknown fields and unsupported conversions fail explicitly.
pub fn render_output_template(template: &str, info: &InfoDict) -> Result<String, CoreError> {
    let mut output = String::new();
    let mut end = 0;
    for captures in OUTPUT_TEMPLATE_RE.captures_iter(template) {
        let whole = captures.get(0).expect("regex capture 0");
        output.push_str(&template[end..whole.start()]);
        let key = captures.name("key").expect("output key").as_str();
        let value = info.get(key).ok_or_else(|| {
            CoreError::new(
                CoreErrorKind::MissingField,
                format!("output template field is missing: {key}"),
            )
        })?;
        output.push_str(&render_template_value(
            value,
            captures.name("format").expect("output format").as_str(),
        )?);
        end = whole.end();
    }
    output.push_str(&template[end..]);
    Ok(output.replace("%%", "%"))
}
