fn karaoketv_player_object(html: &str, variable: &str) -> Option<serde_json::Value> {
    json_object_after_marker(html, &format!("var {variable}"))
}

fn karaoketv_play_path(html: &str) -> Option<String> {
    karaoketv_player_object(html, "options")
        .and_then(|options| options.get("clip").cloned())
        .and_then(|clip| json_string(&clip, "url").map(str::to_owned))
        .filter(|value| !value.is_empty())
}

fn karaoketv_servers(html: &str) -> Vec<String> {
    karaoketv_player_object(html, "settings")
        .and_then(|settings| settings.get("servers").cloned())
        .and_then(|servers| {
            servers.as_array().map(|servers| {
                servers
                    .iter()
                    .filter_map(|server| server.as_str().map(str::to_owned))
                    .filter(|server| !server.is_empty())
                    .collect::<Vec<_>>()
            })
        })
        .filter(|servers| !servers.is_empty())
        .unwrap_or_else(|| vec!["wowzail.video-cdn.com:80/vodcdn".to_owned()])
}

fn karaoketv_formats(
    play_path: &str,
    servers: impl IntoIterator<Item = String>,
    video_cdn_url: &str,
) -> Vec<serde_json::Value> {
    servers
        .into_iter()
        .map(|server| {
            let stream_url = if server.starts_with("rtmp") {
                server
            } else {
                format!("rtmp://{server}")
            };
            serde_json::json!({
                "url": stream_url,
                "play_path": play_path,
                "app": "vodcdn",
                "page_url": video_cdn_url,
                "player_url": "http://www.video-cdn.com/assets/flowplayer/flowplayer.commercial-3.2.18.swf",
                "rtmp_real_time": true,
                "protocol": "rtmp",
                "ext": "flv",
            })
        })
        .collect()
}
