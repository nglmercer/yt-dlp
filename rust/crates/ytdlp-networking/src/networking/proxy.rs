fn no_proxy_matches(url: &Url, no_proxy: &str) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    let port = url.port_or_known_default();

    no_proxy.split(',').any(|entry| {
        let entry = entry.trim().to_ascii_lowercase();
        if entry.is_empty() {
            return false;
        }
        if entry == "*" {
            return true;
        }

        let (entry_host, entry_port) = if let Some(rest) = entry.strip_prefix('[') {
            rest.find(']').map_or((entry.as_str(), None), |end| {
                let host_end = end + 1;
                let port = rest[host_end..]
                    .strip_prefix(':')
                    .and_then(|value| value.parse::<u16>().ok());
                (&entry[..host_end + 1], port)
            })
        } else if entry.matches(':').count() == 1 {
            let (host, port) = entry.rsplit_once(':').unwrap();
            (host, port.parse::<u16>().ok())
        } else {
            (entry.as_str(), None)
        };
        let entry_host = entry_host.trim_matches(['[', ']']);
        let host_matches = host == entry_host
            || host
                .strip_suffix(entry_host)
                .is_some_and(|prefix| prefix.ends_with('.'))
            || (entry_host.starts_with('.') && host.ends_with(entry_host));
        host_matches && entry_port.is_none_or(|entry_port| Some(entry_port) == port)
    })
}

/// Select the proxy for a URL using yt-dlp's per-scheme, `all`, and `no`
/// mapping semantics. Environment proxies are intentionally outside this
/// function so callers can make the result deterministic.
pub fn select_proxy(
    url: &str,
    proxies: &IndexMap<String, Option<String>>,
) -> Result<Option<String>, RequestError> {
    let url = Url::parse(url)
        .map_err(|error| RequestError::invalid(format!("invalid proxy URL: {error}")))?;
    if proxies
        .get("no")
        .and_then(Option::as_deref)
        .is_some_and(|no_proxy| no_proxy_matches(&url, no_proxy))
    {
        return Ok(None);
    }
    if let Some(proxy) = proxies.get(url.scheme()) {
        return Ok(proxy.clone());
    }
    Ok(proxies.get("all").cloned().flatten())
}
