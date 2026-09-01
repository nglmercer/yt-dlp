#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedFormat {
    url: String,
    ext: Option<String>,
    extra_param_to_segment_url: Option<String>,
}

#[cfg(test)]
fn select_download_format(
    info: &InfoDict,
    selector: Option<&str>,
) -> Result<(String, Option<String>), String> {
    let selected = select_download_format_details(info, selector)?;
    Ok((selected.url, selected.ext))
}

fn select_download_format_details(
    info: &InfoDict,
    selector: Option<&str>,
) -> Result<SelectedFormat, String> {
    let formats = format_records(info);
    let Some(selector) = selector else {
        let url = info
            .get("url")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                formats
                    .first()
                    .and_then(|format| format.get("url"))
                    .and_then(serde_json::Value::as_str)
            })
            .ok_or_else(|| "TODO: extractor returned no downloadable native URL".to_owned())?;
        let ext = info
            .get("ext")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let selected_format = formats
            .iter()
            .find(|format| format.get("url").and_then(serde_json::Value::as_str) == Some(url));
        return Ok(SelectedFormat {
            url: url.to_owned(),
            ext,
            extra_param_to_segment_url: selected_format
                .map(|format| format_extra_param(format))
                .transpose()?
                .flatten(),
        });
    };

    let mut selected = None;
    for alternative in selector.split('/') {
        if matches!(alternative, "best" | "b" | "best*") {
            selected = formats.iter().find(|format| format.get("url").is_some());
        } else if matches!(alternative, "bestaudio" | "ba") {
            selected = formats.iter().find(|format| {
                format.get("vcodec").and_then(serde_json::Value::as_str) == Some("none")
            });
        } else if matches!(alternative, "bestvideo" | "bv") {
            selected = formats.iter().find(|format| {
                format.get("vcodec").and_then(serde_json::Value::as_str) != Some("none")
            });
        } else if matches!(alternative, "worst" | "w") {
            selected = formats
                .iter()
                .rev()
                .find(|format| format.get("url").is_some());
        } else if matches!(alternative, "worstaudio" | "wa") {
            selected = formats.iter().rev().find(|format| {
                format.get("vcodec").and_then(serde_json::Value::as_str) == Some("none")
            });
        } else if alternative == "all" {
            return Err("TODO: downloading all native formats is not implemented".to_owned());
        } else if alternative.contains('[')
            || alternative.contains('+')
            || alternative.contains(',')
            || alternative.contains('(')
        {
            return Err(format!(
                "TODO: native format selector syntax is not implemented: {alternative}"
            ));
        } else {
            selected = formats.iter().find(|format| {
                format.get("format_id").and_then(serde_json::Value::as_str) == Some(alternative)
                    || format.get("ext").and_then(serde_json::Value::as_str) == Some(alternative)
            });
        }
        if selected.is_some() {
            break;
        }
    }
    let format =
        selected.ok_or_else(|| format!("no native format matches selector: {selector}"))?;
    let url = format
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "selected native format has no URL".to_owned())?;
    let ext = format
        .get("ext")
        .and_then(serde_json::Value::as_str)
        .or_else(|| info.get("ext").and_then(serde_json::Value::as_str))
        .map(str::to_owned);
    Ok(SelectedFormat {
        url: url.to_owned(),
        ext,
        extra_param_to_segment_url: format_extra_param(format)?,
    })
}

fn format_extra_param(format: &serde_json::Value) -> Result<Option<String>, String> {
    let Some(value) = format.get("extra_param_to_segment_url") else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|value| Some(value.to_owned()))
        .ok_or_else(|| {
            "TODO: native format extra_param_to_segment_url must be a string".to_owned()
        })
}
