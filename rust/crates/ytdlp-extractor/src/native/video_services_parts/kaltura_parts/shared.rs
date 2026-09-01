const KALTURA_SERVICE_URL: &str = "http://cdnapi.kaltura.com";
const KALTURA_SERVICE_BASE: &str = "/api_v3/service/multirequest";

#[derive(Debug, Clone)]
struct KalturaTarget {
    partner_id: String,
    entry_id: String,
    player_type: String,
    ks: Option<String>,
    service_url: String,
}

fn kaltura_target(url: &str) -> Result<KalturaTarget, ExtractorError> {
    if let Some(value) = url.strip_prefix("kaltura:") {
        let mut parts = value.split(':');
        let partner_id = parts.next().filter(|value| !value.is_empty());
        let entry_id = parts.next().filter(|value| !value.is_empty());
        let player_type = parts.next().unwrap_or("html5");
        if let (Some(partner_id), Some(entry_id)) = (partner_id, entry_id) {
            if !matches!(player_type, "html5" | "kwidget") {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!("TODO: unsupported Kaltura player type {player_type}"),
                ));
            }
            return Ok(KalturaTarget {
                partner_id: partner_id.to_owned(),
                entry_id: entry_id.to_owned(),
                player_type: player_type.to_owned(),
                ks: None,
                service_url: KALTURA_SERVICE_URL.to_owned(),
            });
        }
        return Err(ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            "Kaltura URL must contain partner and entry IDs",
        ));
    }

    let parsed = url::Url::parse(url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("invalid Kaltura URL: {error}"),
        )
    })?;
    let mut params = parsed
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    if let Some(path) = parsed.path_segments() {
        let segments = path.filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
        for pair in segments.windows(2) {
            if let [key, value] = pair
                && matches!(*key, "wid" | "p" | "partner_id" | "entry_id" | "uiconf_id")
            {
                params.push(((*key).to_owned(), (*value).to_owned()));
            }
        }
    }
    let partner_id = kaltura_param(&params, "wid")
        .or_else(|| kaltura_param(&params, "p"))
        .or_else(|| kaltura_param(&params, "partner_id"))
        .map(|value| value.trim_start_matches('_').to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Kaltura URL has no partner ID",
            )
        })?;
    let entry_id = kaltura_param(&params, "entry_id").ok_or_else(|| {
        if kaltura_param(&params, "flashvars[playlistAPI.kpl0Id]").is_some() {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                "TODO: Kaltura playlist URLs are not ported to native Rust yet",
            )
        } else if kaltura_param(&params, "flashvars[referenceId]").is_some() {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                "TODO: Kaltura reference-ID URLs are not ported to native Rust yet",
            )
        } else {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Kaltura URL has no entry ID",
            )
        }
    })?;
    let player_type = if parsed.path().contains("html5lib/v2") {
        "kwidget"
    } else {
        "html5"
    };
    Ok(KalturaTarget {
        partner_id,
        entry_id,
        player_type: player_type.to_owned(),
        ks: kaltura_param(&params, "flashvars[ks]"),
        service_url: KALTURA_SERVICE_URL.to_owned(),
    })
}

fn kaltura_param(params: &[(String, String)], key: &str) -> Option<String> {
    params
        .iter()
        .find_map(|(name, value)| (name == key).then(|| value.clone()))
}

fn kaltura_widget_id(partner_id: &str) -> String {
    if partner_id.contains('_') {
        partner_id.to_owned()
    } else {
        format!("_{partner_id}")
    }
}

fn kaltura_partner_value(partner_id: &str) -> serde_json::Value {
    partner_id
        .parse::<i64>()
        .map_or_else(|_| serde_json::json!(partner_id), |value| serde_json::json!(value))
}

fn kaltura_signed_url(base: &str, ks: Option<&str>) -> String {
    let base = base.trim_end_matches('/');
    ks.map_or_else(|| base.to_owned(), |ks| format!("{base}/ks/{ks}"))
}
