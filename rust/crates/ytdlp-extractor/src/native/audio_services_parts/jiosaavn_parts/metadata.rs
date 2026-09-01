fn jiosaavn_value<'a>(item: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    item.get(key)
        .or_else(|| item.get("more_info").and_then(|value| value.get(key)))
}

fn jiosaavn_clean_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .map(html_text_fragment)
        .filter(|value| !value.is_empty())
}

fn jiosaavn_raw_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .filter(|value| !value.is_empty())
}

fn jiosaavn_integer(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_f64().map(|value| value as i64))
            .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
    })
}

fn jiosaavn_absolute_url(value: &str) -> String {
    resolve_url("https://www.jiosaavn.com/", value)
}

fn jiosaavn_valid_url(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value.and_then(serde_json::Value::as_str)?;
    (value.starts_with("http://") || value.starts_with("https://")).then(|| value.to_owned())
}

fn jiosaavn_url_basename(value: &str) -> String {
    value
        .split(['?', '#'])
        .next()
        .unwrap_or(value)
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(value)
        .to_owned()
}

fn jiosaavn_thumbnail(value: &str) -> String {
    Regex::new(r"-\d+x\d+\.")
        .ok()
        .map(|matcher| matcher.replace(value, "-500x500.").into_owned())
        .unwrap_or_else(|| value.to_owned())
}

fn jiosaavn_release_date(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value.and_then(serde_json::Value::as_str)?;
    date_digits(value)
}

fn jiosaavn_language(value: Option<&serde_json::Value>) -> Option<String> {
    let code = value
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_ascii_lowercase();
    let short = code.get(..2).unwrap_or(code.as_str());
    let language = match short {
        "af" => "afr",
        "ar" => "ara",
        "as" => "asm",
        "az" => "aze",
        "be" => "bel",
        "bg" => "bul",
        "bn" => "ben",
        "bs" => "bos",
        "ca" => "cat",
        "cs" => "ces",
        "cy" => "cym",
        "da" => "dan",
        "de" => "deu",
        "el" => "ell",
        "en" => "eng",
        "es" => "spa",
        "et" => "est",
        "eu" => "eus",
        "fa" => "fas",
        "fi" => "fin",
        "fr" => "fra",
        "ga" => "gle",
        "gl" => "glg",
        "gu" => "guj",
        "he" => "heb",
        "hi" => "hin",
        "hr" => "hrv",
        "hu" => "hun",
        "hy" => "hye",
        "id" => "ind",
        "is" => "isl",
        "it" => "ita",
        "ja" => "jpn",
        "ka" => "kat",
        "kk" => "kaz",
        "km" => "khm",
        "kn" => "kan",
        "ko" => "kor",
        "lt" => "lit",
        "lv" => "lav",
        "mk" => "mkd",
        "ml" => "mal",
        "mn" => "mon",
        "mr" => "mar",
        "ms" => "msa",
        "my" => "mya",
        "ne" => "nep",
        "nl" => "nld",
        "no" => "nor",
        "or" => "ori",
        "pa" => "pan",
        "pl" => "pol",
        "pt" => "por",
        "ro" => "ron",
        "ru" => "rus",
        "sa" => "san",
        "sk" => "slk",
        "sl" => "slv",
        "so" => "som",
        "sq" => "sqi",
        "sr" => "srp",
        "sv" => "swe",
        "sw" => "swa",
        "ta" => "tam",
        "te" => "tel",
        "th" => "tha",
        "tr" => "tur",
        "uk" => "ukr",
        "ur" => "urd",
        "uz" => "uzb",
        "vi" => "vie",
        "yi" => "yid",
        "zh" => "zho",
        "zu" => "zul",
        _ => "und",
    };
    Some(language.to_owned())
}

fn jiosaavn_timestamp(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    if let Some(timestamp) = jiosaavn_integer(Some(value)) {
        return Some(timestamp);
    }
    let value = value.as_str()?.trim();
    parse_timestamp(value.to_owned()).or_else(|| {
        let date = date_digits(value)?;
        let normalized = format!(
            "{}-{}-{}T00:00:00Z",
            &date[..4],
            &date[4..6],
            &date[6..8]
        );
        parse_timestamp(normalized)
    })
}

fn jiosaavn_extract_song_info(item: &serde_json::Value, webpage_url: Option<&str>) -> InfoDict {
    let mut info = InfoDict::new();
    if let Some(id) = json_value_string(item.get("id")) {
        info.insert("id", serde_json::json!(id));
    }
    let title = jiosaavn_clean_string(
        item.get("song")
            .and_then(|song| song.get("title"))
            .or_else(|| jiosaavn_value(item, "title")),
    );
    info.insert_if_some("title", title);
    info.insert_if_some("album", jiosaavn_clean_string(jiosaavn_value(item, "album")));
    info.insert_if_some("duration", jiosaavn_integer(jiosaavn_value(item, "duration")));
    info.insert_if_some("channel", jiosaavn_raw_string(jiosaavn_value(item, "label")));
    info.insert_if_some("channel_id", jiosaavn_raw_string(jiosaavn_value(item, "label_id")));
    if let Some(label_url) = jiosaavn_raw_string(jiosaavn_value(item, "label_url")) {
        info.insert("channel_url", serde_json::json!(jiosaavn_absolute_url(&label_url)));
    }
    info.insert_if_some(
        "release_date",
        jiosaavn_release_date(jiosaavn_value(item, "release_date")),
    );
    info.insert_if_some("release_year", jiosaavn_integer(item.get("year")));
    info.insert_if_some(
        "thumbnail",
        jiosaavn_valid_url(item.get("image")).map(|value| jiosaavn_thumbnail(&value)),
    );
    info.insert_if_some("view_count", jiosaavn_integer(item.get("play_count")));
    info.insert_if_some("language", jiosaavn_language(item.get("language")));

    let resolved_webpage_url = jiosaavn_valid_url(item.get("perma_url"))
        .or_else(|| webpage_url.map(str::to_owned));
    if let Some(webpage_url) = resolved_webpage_url {
        let display_id = jiosaavn_url_basename(&webpage_url);
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("webpage_url", serde_json::json!(webpage_url));
        info.insert(
            "_old_archive_ids",
            serde_json::json!([format!("jiosaavnsong {display_id}")]),
        );
    }

    let mut artists = Vec::new();
    if let Some(primary_artists) = item
        .get("more_info")
        .and_then(|more_info| more_info.get("artistMap"))
        .and_then(|artist_map| artist_map.get("primary_artists"))
        .and_then(serde_json::Value::as_array)
    {
        for artist in primary_artists {
            if let Some(name) = json_string(artist, "name") {
                artists.push(name.to_owned());
            }
        }
    }
    for key in ["primary_artists", "featured_artists"] {
        if let Some(value) = item.get(key).and_then(serde_json::Value::as_str) {
            artists.extend(value.split(", ").map(str::to_owned));
        }
    }
    artists.retain(|artist| !artist.is_empty());
    let mut unique_artists = Vec::new();
    for artist in artists {
        if !unique_artists.contains(&artist) {
            unique_artists.push(artist);
        }
    }
    info.insert_if_some("artists", (!unique_artists.is_empty()).then_some(unique_artists));
    info
}

fn jiosaavn_extract_episode_info(item: &serde_json::Value, webpage_url: Option<&str>) -> InfoDict {
    let mut info = jiosaavn_extract_song_info(item, webpage_url);
    info.remove("_old_archive_ids");
    info.insert_if_some(
        "description",
        jiosaavn_raw_string(item.get("more_info").and_then(|value| value.get("description"))),
    );
    info.insert_if_some(
        "timestamp",
        jiosaavn_timestamp(item.get("more_info").and_then(|value| value.get("release_time"))),
    );
    let more_info = item.get("more_info");
    info.insert_if_some(
        "series",
        jiosaavn_raw_string(more_info.and_then(|value| value.get("show_title"))),
    );
    info.insert_if_some(
        "series_id",
        jiosaavn_raw_string(more_info.and_then(|value| value.get("show_id"))),
    );
    info.insert_if_some(
        "season",
        jiosaavn_raw_string(more_info.and_then(|value| value.get("season_title"))),
    );
    info.insert_if_some(
        "season_number",
        jiosaavn_integer(more_info.and_then(|value| value.get("season_no"))),
    );
    info.insert_if_some(
        "season_id",
        jiosaavn_raw_string(more_info.and_then(|value| value.get("season_id"))),
    );
    info.insert_if_some(
        "episode_number",
        jiosaavn_integer(more_info.and_then(|value| value.get("episode_number"))),
    );
    info.insert_if_some(
        "cast",
        item.get("starring")
            .and_then(serde_json::Value::as_str)
            .map(|value| value.split(", ").map(str::to_owned).collect::<Vec<_>>())
            .filter(|value| !value.is_empty()),
    );
    info
}
