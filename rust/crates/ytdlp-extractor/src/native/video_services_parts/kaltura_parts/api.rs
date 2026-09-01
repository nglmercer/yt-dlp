fn kaltura_fetch_video(
    context: &ExtractionContext,
    target: &KalturaTarget,
) -> Result<(serde_json::Value, serde_json::Value, serde_json::Value), ExtractorError> {
    let actions = kaltura_actions(target);
    let mut payload = serde_json::Map::new();
    let Some(header) = actions.first().and_then(serde_json::Value::as_object) else {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Kaltura action list has no multirequest header",
        ));
    };
    payload.extend(header.clone());
    for (index, action) in actions.iter().skip(1).enumerate() {
        payload.insert((index + 1).to_string(), action.clone());
    }
    let endpoint = format!(
        "{}{}",
        target.service_url.trim_end_matches('/'),
        KALTURA_SERVICE_BASE
    );
    let response = native_post_json(context, &endpoint, &serde_json::Value::Object(payload))?;
    let response_items = response.as_array().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Kaltura multirequest response is not an array",
        )
    })?;
    for (index, item) in response_items.iter().enumerate() {
        if json_string(item, "objectType") == Some("KalturaAPIException") {
            let message = json_string(item, "message").unwrap_or("Kaltura API error");
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kaltura API error at request {index}: {message}"),
            ));
        }
    }
    let info_index = if target.player_type == "kwidget" { 3 } else { 2 };
    let info = kaltura_response_first_object(&response, info_index).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Kaltura entry {} has no metadata object", target.entry_id),
        )
    })?;
    let flavor_assets = kaltura_response_value(&response, info_index + 1)
        .cloned()
        .unwrap_or_else(|| serde_json::json!({"objects": []}));
    let captions = kaltura_response_value(&response, info_index + 2)
        .cloned()
        .unwrap_or_else(|| serde_json::json!({"objects": []}));
    Ok((info, flavor_assets, captions))
}

fn kaltura_response_value(
    response: &serde_json::Value,
    index: usize,
) -> Option<&serde_json::Value> {
    response.as_array().and_then(|values| values.get(index))
}

fn kaltura_response_first_object(
    response: &serde_json::Value,
    index: usize,
) -> Option<serde_json::Value> {
    kaltura_response_value(response, index)?
        .get("objects")
        .and_then(serde_json::Value::as_array)
        .and_then(|objects| objects.first())
        .cloned()
}

fn kaltura_actions(target: &KalturaTarget) -> Vec<serde_json::Value> {
    let partner_id = kaltura_partner_value(&target.partner_id);
    let widget_id = kaltura_widget_id(&target.partner_id);
    let info_action = serde_json::json!({
        "action": "list",
        "filter": {"redirectFromEntryId": target.entry_id},
        "service": "baseentry",
        "ks": "{1:result:ks}",
        "responseProfile": {
            "type": 1,
            "fields": "createdAt,dataUrl,duration,name,plays,thumbnailUrl,userId,description"
        }
    });
    let flavor_action = serde_json::json!({
        "action": "getbyentryid",
        "entryId": target.entry_id,
        "service": "flavorAsset",
        "ks": "{1:result:ks}"
    });
    let captions_action = serde_json::json!({
        "action": "list",
        "filter:entryIdEqual": target.entry_id,
        "service": "caption_captionasset",
        "ks": "{1:result:ks}"
    });
    if target.player_type == "kwidget" {
        vec![
            serde_json::json!({
                "service": "multirequest",
                "apiVersion": "3.1",
                "expiry": 86400,
                "clientTag": "kwidget:v2.89",
                "format": 1,
                "ignoreNull": 1,
                "action": "null"
            }),
            serde_json::json!({
                "expiry": 86400,
                "service": "session",
                "action": "startWidgetSession",
                "widgetId": widget_id
            }),
            serde_json::json!({
                "expiry": 86400,
                "service": "session",
                "action": "startwidgetsession",
                "widgetId": widget_id,
                "format": 9,
                "apiVersion": "3.1",
                "clientTag": "kwidget:v2.89",
                "ignoreNull": 1,
                "ks": "{1:result:ks}"
            }),
            info_action,
            flavor_action,
            captions_action,
        ]
    } else {
        vec![
            serde_json::json!({
                "apiVersion": "3.3.0",
                "clientTag": "html5:v3.1.0",
                "format": 1,
                "ks": "",
                "partnerId": partner_id
            }),
            serde_json::json!({
                "expiry": 86400,
                "service": "session",
                "action": "startWidgetSession",
                "widgetId": widget_id
            }),
            info_action,
            flavor_action,
            captions_action,
        ]
    }
}
