//! Bounded HTTP framing for the existing Website supervisor/admin contract.
//! This module reads and writes bytes only; `host/game-session-supervisor.ts` owns
//! the JSON routes, bearer authentication and their resulting authority changes.

use std::io::{self, BufRead, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use super::{ServerState, TransportEvent};

const MAX_PROVISION_REQUEST_BYTES: usize = 48 * 1024 * 1024;
// Source recovery permits a complete browser save in addition to admitted packages.
const MAX_REJOIN_REQUEST_BYTES: usize =
    MAX_PROVISION_REQUEST_BYTES + solomon_protocol::MAX_SAVE_BYTES * 2 + 64 * 1024;
const MAX_HTTP_LINE_BYTES: usize = 16 * 1024;

pub struct HostedHttpRequest {
    pub method: String,
    pub path: String,
    pub authorization: Option<String>,
    pub deployment_id: Option<String>,
    pub body: Vec<u8>,
    pub remote_addr: SocketAddr,
}

pub struct HostedHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HostedHttpResponse {
    #[must_use]
    pub fn empty(status: u16) -> Self {
        Self { status, headers: Vec::new(), body: Vec::new() }
    }
}

pub(super) fn hosted_websocket_path(path: &str) -> bool {
    if path == solomon_protocol::hosted::GAME_HUB_PATH || path == "/game" {
        return true;
    }
    let Some(id) = path.strip_prefix(solomon_protocol::hosted::GAME_SESSION_PATH_PREFIX) else {
        return false;
    };
    id.len() == 32
        && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub(super) fn serve(
    stream: &mut (impl io::Read + Write),
    remote_addr: SocketAddr,
    server: &Arc<ServerState>,
) -> io::Result<()> {
    let mut reader = io::BufReader::new(stream);
    let request = match read_request(&mut reader, remote_addr) {
        Ok(request) => request,
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return write_response(reader.get_mut(), &HostedHttpResponse::empty(400));
        }
        Err(error) => return Err(error),
    };
    let (respond, response) = mpsc::sync_channel(1);
    if server.events.send(TransportEvent::Http { request, respond }).is_err() {
        return Ok(());
    }
    loop {
        match response.recv_timeout(Duration::from_millis(100)) {
            Ok(response) => return write_response(reader.get_mut(), &response),
            Err(RecvTimeoutError::Disconnected) => {
                return write_response(reader.get_mut(), &HostedHttpResponse::empty(503));
            }
            Err(RecvTimeoutError::Timeout) => {
                if server.stopping.load(Ordering::Acquire) {
                    return Ok(());
                }
            }
        }
    }
}

fn read_request(
    reader: &mut impl BufRead,
    remote_addr: SocketAddr,
) -> io::Result<HostedHttpRequest> {
    let line = read_line(reader)?;
    let mut fields = line.split_whitespace();
    let (Some(method), Some(target), Some(version), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Err(malformed());
    };
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") || !target.starts_with('/') {
        return Err(malformed());
    }
    let method = method.to_owned();
    let path = target.split('?').next().expect("a request target is present").to_owned();
    let maximum_body = if path == "/admin/rejoin" {
        MAX_REJOIN_REQUEST_BYTES
    } else {
        MAX_PROVISION_REQUEST_BYTES
    };
    let mut authorization = None;
    let mut deployment_id = None;
    let mut content_length = None;
    let mut chunked = false;
    let mut header_bytes = line.len();
    loop {
        let line = read_line(reader)?;
        header_bytes += line.len();
        if header_bytes > MAX_HTTP_LINE_BYTES {
            return Err(malformed());
        }
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').ok_or_else(malformed)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("authorization") {
            if authorization.replace(value.to_owned()).is_some() {
                return Err(malformed());
            }
        } else if name.eq_ignore_ascii_case("x-solomon-deployment-id") {
            if deployment_id.replace(value.to_owned()).is_some() {
                return Err(malformed());
            }
        } else if name.eq_ignore_ascii_case("content-length") {
            let length: usize = value.parse().map_err(|_| malformed())?;
            if content_length.replace(length).is_some() || length > maximum_body {
                return Err(malformed());
            }
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            if chunked || !value.eq_ignore_ascii_case("chunked") {
                return Err(malformed());
            }
            chunked = true;
        }
    }
    // Do not permit ambiguous request framing at this proxy-facing trust boundary.
    if chunked && content_length.is_some() {
        return Err(malformed());
    }
    let mut body = Vec::new();
    if chunked {
        loop {
            let line = read_line(reader)?;
            let length = usize::from_str_radix(line.split(';').next().unwrap_or(""), 16)
                .map_err(|_| malformed())?;
            if length == 0 {
                let mut trailer_bytes = 0;
                loop {
                    let trailer = read_line(reader)?;
                    trailer_bytes += trailer.len();
                    if trailer_bytes > MAX_HTTP_LINE_BYTES {
                        return Err(malformed());
                    }
                    if trailer.is_empty() {
                        break;
                    }
                }
                break;
            }
            if length > maximum_body.saturating_sub(body.len()) {
                return Err(malformed());
            }
            let start = body.len();
            body.resize(start + length, 0);
            reader.read_exact(&mut body[start..])?;
            if !read_line(reader)?.is_empty() {
                return Err(malformed());
            }
        }
    } else if let Some(length) = content_length {
        body.resize(length, 0);
        reader.read_exact(&mut body)?;
    }
    Ok(HostedHttpRequest { method, path, authorization, deployment_id, body, remote_addr })
}

fn read_line(reader: &mut impl BufRead) -> io::Result<String> {
    let mut bytes = Vec::new();
    let read = reader.take((MAX_HTTP_LINE_BYTES + 1) as u64).read_until(b'\n', &mut bytes)?;
    if read == 0 || bytes.len() > MAX_HTTP_LINE_BYTES || !bytes.ends_with(b"\r\n") {
        return Err(malformed());
    }
    bytes.truncate(bytes.len() - 2);
    String::from_utf8(bytes).map_err(|_| malformed())
}

fn malformed() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "malformed supervisor HTTP request")
}

fn write_response(writer: &mut impl Write, response: &HostedHttpResponse) -> io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Internal Server Error",
    };
    write!(
        writer,
        "HTTP/1.1 {} {reason}\r\nConnection: close\r\nContent-Length: {}\r\nCache-Control: no-store\r\n",
        response.status,
        response.body.len()
    )?;
    for (name, value) in &response.headers {
        write!(writer, "{name}: {value}\r\n")?;
    }
    writer.write_all(b"\r\n")?;
    writer.write_all(&response.body)?;
    writer.flush()
}
