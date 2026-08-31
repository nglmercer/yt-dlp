fn parse_http_response(request_url: &str, raw: &[u8]) -> Result<Response, RequestError> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| {
            RequestError::new(
                ErrorKind::Transport,
                "HTTP response has no header terminator",
            )
        })?;
    let header_text = std::str::from_utf8(&raw[..header_end]).map_err(|error| {
        RequestError::new(
            ErrorKind::Transport,
            format!("invalid HTTP headers: {error}"),
        )
    })?;
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().ok_or_else(|| {
        RequestError::new(ErrorKind::Transport, "HTTP response has no status line")
    })?;
    let mut status_parts = status_line.splitn(3, ' ');
    let version = status_parts.next().unwrap_or_default();
    let status = status_parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| {
            RequestError::new(ErrorKind::Transport, "HTTP response has an invalid status")
        })?;
    if !version.starts_with("HTTP/") {
        return Err(RequestError::new(
            ErrorKind::Transport,
            "HTTP response has an invalid version",
        ));
    }

    let mut headers = ResponseHeaders::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or_else(|| {
            RequestError::new(ErrorKind::Transport, "HTTP response has an invalid header")
        })?;
        headers.add(name, value);
    }

    let encoded_body = &raw[header_end + 4..];
    let body = if headers
        .get("Transfer-Encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        decode_chunked_body(encoded_body)?
    } else if let Some(length) = headers.get("Content-Length") {
        let length = length.parse::<usize>().map_err(|error| {
            RequestError::new(
                ErrorKind::Transport,
                format!("invalid Content-Length: {error}"),
            )
        })?;
        if encoded_body.len() < length {
            return Err(RequestError::new(
                ErrorKind::Transport,
                "HTTP response body is incomplete",
            ));
        }
        encoded_body[..length].to_vec()
    } else {
        encoded_body.to_vec()
    };

    let reason = status_parts_reason(status_line)
        .filter(|reason| !reason.is_empty())
        .unwrap_or_else(|| default_http_reason(status));
    let mut response = Response::new(request_url, status, reason, body);
    response.headers = headers;
    Ok(response)
}

fn status_parts_reason(status_line: &str) -> Option<&str> {
    status_line.splitn(3, ' ').nth(2)
}

fn default_http_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}

fn decode_chunked_body(mut body: &[u8]) -> Result<Vec<u8>, RequestError> {
    let mut decoded = Vec::new();
    loop {
        let line_end = body
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| RequestError::new(ErrorKind::Transport, "invalid chunk size"))?;
        let size_text = std::str::from_utf8(&body[..line_end]).map_err(|error| {
            RequestError::new(ErrorKind::Transport, format!("invalid chunk size: {error}"))
        })?;
        let size_text = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|error| {
            RequestError::new(ErrorKind::Transport, format!("invalid chunk size: {error}"))
        })?;
        body = &body[line_end + 2..];
        if size == 0 {
            return Ok(decoded);
        }
        let end = size
            .checked_add(2)
            .ok_or_else(|| RequestError::new(ErrorKind::Transport, "chunk body is too large"))?;
        if body.len() < end || &body[size..end] != b"\r\n" {
            return Err(RequestError::new(
                ErrorKind::Transport,
                "invalid chunk body",
            ));
        }
        decoded.extend_from_slice(&body[..size]);
        body = &body[end..];
    }
}

fn chunked_body_end(body: &[u8]) -> Result<Option<usize>, RequestError> {
    let mut offset = 0;
    loop {
        let Some(line_end) = body[offset..]
            .windows(2)
            .position(|window| window == b"\r\n")
        else {
            return Ok(None);
        };
        let line_end = offset + line_end;
        let size_text = std::str::from_utf8(&body[offset..line_end]).map_err(|error| {
            RequestError::new(ErrorKind::Transport, format!("invalid chunk size: {error}"))
        })?;
        let size_text = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|error| {
            RequestError::new(ErrorKind::Transport, format!("invalid chunk size: {error}"))
        })?;
        let data_start = line_end + 2;
        if size == 0 {
            let trailers = &body[data_start..];
            if trailers.starts_with(b"\r\n") {
                return Ok(Some(data_start + 2));
            }
            return Ok(trailers
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|end| data_start + end + 4));
        }
        let data_end = data_start
            .checked_add(size)
            .ok_or_else(|| RequestError::new(ErrorKind::Transport, "chunk body is too large"))?;
        let chunk_end = data_end
            .checked_add(2)
            .ok_or_else(|| RequestError::new(ErrorKind::Transport, "chunk body is too large"))?;
        if body.len() < chunk_end {
            return Ok(None);
        }
        if &body[data_end..chunk_end] != b"\r\n" {
            return Err(RequestError::new(
                ErrorKind::Transport,
                "invalid chunk body",
            ));
        }
        offset = chunk_end;
    }
}
