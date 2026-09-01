#[derive(Default)]
struct MdrDocument {
    title: Option<String>,
    media_type: Option<String>,
    duration: Option<String>,
    description: Option<String>,
    uploader: Option<String>,
    broadcast_name: Option<String>,
    broadcast_description: Option<String>,
    broadcast_date: Option<String>,
    broadcast_start_date: Option<String>,
    broadcast_end_date: Option<String>,
    assets: Vec<MdrAsset>,
}

#[derive(Default)]
struct MdrAsset {
    download_url: Option<String>,
    progressive_download_url: Option<String>,
    dynamic_streaming_url: Option<String>,
    adaptive_streaming_url: Option<String>,
    media_type: Option<String>,
    bitrate_video: Option<String>,
    bitrate_audio: Option<String>,
    file_size: Option<String>,
    frame_width: Option<String>,
    frame_height: Option<String>,
}

fn mdr_parse_xml(body: &[u8]) -> Result<MdrDocument, ExtractorError> {
    let mut reader = quick_xml::Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut path = Vec::new();
    let mut document = MdrDocument::default();
    let mut asset = None;
    loop {
        buffer.clear();
        let event = reader.read_event_into(&mut buffer).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid MDR XML configuration: {error}"),
            )
        })?;
        match event {
            quick_xml::events::Event::Start(start) => {
                let name = mdr_xml_name(start.name().as_ref());
                if name == "asset" {
                    asset = Some(MdrAsset::default());
                }
                path.push(name);
            }
            quick_xml::events::Event::Empty(empty) => {
                if mdr_xml_name(empty.name().as_ref()) == "asset" {
                    document.assets.push(MdrAsset::default());
                }
            }
            quick_xml::events::Event::Text(text) => {
                let value = mdr_xml_text(text.as_ref());
                mdr_append_xml_text(&mut document, &mut asset, &path, &value);
            }
            quick_xml::events::Event::CData(text) => {
                let value = String::from_utf8_lossy(text.as_ref()).into_owned();
                mdr_append_xml_text(&mut document, &mut asset, &path, &value);
            }
            quick_xml::events::Event::End(end) => {
                let name = mdr_xml_name(end.name().as_ref());
                if name == "asset" {
                    if let Some(asset) = asset.take() {
                        document.assets.push(asset);
                    }
                }
                path.pop();
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    Ok(document)
}

fn mdr_append_xml_text(
    document: &mut MdrDocument,
    asset: &mut Option<MdrAsset>,
    path: &[String],
    value: &str,
) {
    let Some(field) = path.last().map(String::as_str) else {
        return;
    };
    if let Some(asset) = asset.as_mut() {
        let target = match field {
            "downloadUrl" => &mut asset.download_url,
            "progressiveDownloadUrl" => &mut asset.progressive_download_url,
            "dynamicHttpStreamingRedirectorUrl" => &mut asset.dynamic_streaming_url,
            "adaptiveHttpStreamingRedirectorUrl" => &mut asset.adaptive_streaming_url,
            "mediaType" => &mut asset.media_type,
            "bitrateVideo" => &mut asset.bitrate_video,
            "bitrateAudio" => &mut asset.bitrate_audio,
            "fileSize" => &mut asset.file_size,
            "frameWidth" => &mut asset.frame_width,
            "frameHeight" => &mut asset.frame_height,
            _ => return,
        };
        target.get_or_insert_with(String::new).push_str(value);
        return;
    }
    let target = match field {
        "title" => &mut document.title,
        "type" => &mut document.media_type,
        "duration" => &mut document.duration,
        "description" => &mut document.description,
        "rights" => &mut document.uploader,
        "broadcastName" => &mut document.broadcast_name,
        "broadcastDescription" => &mut document.broadcast_description,
        "broadcastDate" => &mut document.broadcast_date,
        "broadcastStartDate" => &mut document.broadcast_start_date,
        "broadcastEndDate" => &mut document.broadcast_end_date,
        _ => return,
    };
    target.get_or_insert_with(String::new).push_str(value);
}

fn mdr_xml_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name)
        .rsplit_once(':')
        .map_or_else(|| String::from_utf8_lossy(name).into_owned(), |(_, name)| name.to_owned())
}

fn mdr_xml_text(value: &[u8]) -> String {
    let value = String::from_utf8_lossy(value);
    quick_xml::escape::unescape(&value)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| value.into_owned())
}
