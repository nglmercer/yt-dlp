fn jiosaavn_first_item<'a>(
    response: &'a serde_json::Value,
    key: &str,
    endpoint: &str,
) -> Result<&'a serde_json::Value, ExtractorError> {
    response
        .get(key)
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JioSaavn {endpoint} response has no {key} item"),
            )
        })
}

fn jiosaavn_item_entry(item: &serde_json::Value, episode: bool) -> Option<InfoDict> {
    let target_url = jiosaavn_valid_url(item.get("perma_url"))?;
    let metadata = if episode {
        jiosaavn_extract_episode_info(item, None)
    } else {
        jiosaavn_extract_song_info(item, None)
    };
    let mut entry = native_url_result(&target_url);
    entry.insert("_type", serde_json::json!("url_transparent"));
    entry.insert(
        "ie_key",
        serde_json::json!(if episode { "JioSaavnShow" } else { "JioSaavnSong" }),
    );
    for (key, value) in metadata.iter() {
        entry.insert(key, value.clone());
    }
    // The entry owns its metadata; the target extractor refreshes formats
    // through the same native API when the CLI resolves this URL result.
    Some(entry)
}

fn jiosaavn_items<'a>(
    value: &'a serde_json::Value,
    key: Option<&str>,
) -> Vec<&'a serde_json::Value> {
    let value = key.and_then(|key| value.get(key)).unwrap_or(value);
    if let Some(items) = value.as_array() {
        return items.iter().collect();
    }
    let mut items = Vec::new();
    jiosaavn_collect_items(value, &mut items);
    items
}

fn jiosaavn_collect_items<'a>(
    value: &'a serde_json::Value,
    items: &mut Vec<&'a serde_json::Value>,
) {
    match value {
        serde_json::Value::Object(object) => {
            if object.contains_key("id") && object.contains_key("perma_url") {
                items.push(value);
                return;
            }
            for child in object.values() {
                jiosaavn_collect_items(child, items);
            }
        }
        serde_json::Value::Array(array) => {
            for child in array {
                jiosaavn_collect_items(child, items);
            }
        }
        _ => {}
    }
}
