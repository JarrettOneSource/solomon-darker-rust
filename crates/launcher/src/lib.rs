//! Local delivery of the packaged WebAssembly client. HTTP file and byte-range semantics
//! follow RFC 9110; the game rules remain in `solomon-sim`.

#[cfg(not(target_arch = "wasm32"))]
pub mod host;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport;

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

const MAX_REQUEST_BYTES: u64 = 16 * 1024;

/// The immutable directory shipped beside the native launcher.
pub struct AssetServer {
    root: PathBuf,
}

impl AssetServer {
    pub fn new(root: &Path) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        if !root.join("index.html").is_file() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "game/index.html is missing"));
        }
        Ok(Self { root })
    }

    /// The transport dispatcher replays its inspected headers through this same responder.
    /// Static assets therefore keep one implementation of MIME, ranges and path validation.
    pub(crate) fn serve_stream(&self, mut stream: &mut (impl Read + Write)) -> io::Result<()> {
        let mut reader = BufReader::new((&mut stream).take(MAX_REQUEST_BYTES));
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let mut fields = line.split_whitespace();
        let (Some(method), Some(target), Some(version), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return send_error(&mut stream, "400 Bad Request", "Malformed request", false, "");
        };
        let head = method == "HEAD";
        if method != "GET" && !head {
            return send_error(
                &mut stream,
                "405 Method Not Allowed",
                "Use GET or HEAD",
                false,
                "Allow: GET, HEAD\r\n",
            );
        }
        if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
            return send_error(
                &mut stream,
                "400 Bad Request",
                "Unsupported HTTP version",
                head,
                "",
            );
        }
        let path = asset_path(target);
        let mut range = None;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header)? == 0 {
                return send_error(&mut stream, "400 Bad Request", "Incomplete headers", head, "");
            }
            if header == "\r\n" || header == "\n" {
                break;
            }
            let Some((name, value)) = header.split_once(':') else {
                return send_error(&mut stream, "400 Bad Request", "Malformed header", head, "");
            };
            if name.eq_ignore_ascii_case("range") {
                range = Some(value.trim().to_owned());
            }
        }
        let Some(path) = path else {
            return send_error(&mut stream, "400 Bad Request", "Invalid asset path", head, "");
        };
        let path = match fs::canonicalize(self.root.join(path)) {
            Ok(path) if path.starts_with(&self.root) && path.is_file() => path,
            _ => return send_error(&mut stream, "404 Not Found", "Asset not found", head, ""),
        };
        let mut file = File::open(&path)?;
        let length = file.metadata()?.len();
        // RFC 9110 only applies Range to GET. Unsupported multi-range requests receive
        // the whole file, which also keeps the launcher free of multipart machinery.
        let bytes = if head { None } else { range.as_deref().and_then(|r| byte_range(r, length)) };
        let (status, offset, count, extra) = match bytes {
            None => ("200 OK", 0, length, String::new()),
            Some(Ok((start, end))) => (
                "206 Partial Content",
                start,
                end - start + 1,
                format!("Content-Range: bytes {start}-{end}/{length}\r\n"),
            ),
            Some(Err(())) => {
                return send_error(
                    &mut stream,
                    "416 Range Not Satisfiable",
                    "Requested bytes are outside the asset",
                    head,
                    &format!("Content-Range: bytes */{length}\r\n"),
                );
            }
        };
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {}\r\nContent-Length: {count}\r\n\
             Accept-Ranges: bytes\r\nCache-Control: no-cache\r\n\
             X-Content-Type-Options: nosniff\r\nConnection: close\r\n{extra}\r\n",
            mime_type(&path)
        )?;
        if !head {
            file.seek(SeekFrom::Start(offset))?;
            io::copy(&mut file.take(count), &mut stream)?;
        }
        Ok(())
    }
}

fn asset_path(target: &str) -> Option<PathBuf> {
    let target = target.split('?').next()?;
    if !target.starts_with('/') {
        return None;
    }
    let mut decoded = Vec::with_capacity(target.len());
    let mut bytes = target.bytes();
    while let Some(byte) = bytes.next() {
        let byte = if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        };
        if byte == b'\\' || byte == 0 {
            return None;
        }
        decoded.push(byte);
    }
    let decoded = String::from_utf8(decoded).ok()?;
    let path = Path::new(decoded.trim_start_matches('/'));
    if path.components().any(|part| !matches!(part, Component::Normal(_))) {
        return None;
    }
    Some(if path.as_os_str().is_empty() { PathBuf::from("index.html") } else { path.to_path_buf() })
}

fn byte_range(value: &str, length: u64) -> Option<Result<(u64, u64), ()>> {
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if end.contains(',') {
        return None;
    }
    if length == 0 {
        return Some(Err(()));
    }
    if start.is_empty() {
        let suffix: u64 = end.parse().ok()?;
        return Some(if suffix == 0 {
            Err(())
        } else {
            Ok((length.saturating_sub(suffix), length - 1))
        });
    }
    let start: u64 = start.parse().ok()?;
    let end = if end.is_empty() { length - 1 } else { end.parse::<u64>().ok()?.min(length - 1) };
    Some(if start >= length || end < start { Err(()) } else { Ok((start, end)) })
}

fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

fn send_error(
    stream: &mut impl Write,
    status: &str,
    message: &str,
    head: bool,
    extra: &str,
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n{extra}\r\n",
        message.len()
    )?;
    if !head {
        stream.write_all(message.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::AssetServer;
    use crate::transport::{NativeServer, TransportConfig};

    struct TestAssets(PathBuf);

    impl Drop for TestAssets {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("the temporary assets remain writable");
        }
    }

    fn request(server: &AssetServer, request: &str) -> Vec<u8> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let mut native = NativeServer::start(
            listener,
            AssetServer::new(&server.root).unwrap(),
            TransportConfig::default(),
        )
        .unwrap();
        let mut stream = TcpStream::connect(native.local_addr()).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        native.shutdown();
        bytes
    }

    #[test]
    fn packaged_assets_keep_mime_ranges_and_paths_correct_over_real_http() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let assets = TestAssets(std::env::temp_dir().join(format!("solomon-assets-{unique}")));
        fs::create_dir(&assets.0).unwrap();
        fs::write(assets.0.join("index.html"), "<title>Solomon Darker</title>").unwrap();
        fs::write(assets.0.join("game.wasm"), b"\0asm0123456789").unwrap();
        fs::write(assets.0.join("theme.mp3"), b"abcdefghij").unwrap();
        let server = AssetServer::new(&assets.0).unwrap();

        let home = request(&server, "GET /?launch=1 HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(home.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(home.ends_with(b"<title>Solomon Darker</title>"));
        let wasm = request(&server, "GET /game.wasm HTTP/1.1\r\n\r\n");
        assert!(String::from_utf8_lossy(&wasm).contains("Content-Type: application/wasm\r\n"));
        assert!(wasm.ends_with(b"\0asm0123456789"));

        let partial = request(&server, "GET /theme.mp3 HTTP/1.1\r\nRange: bytes=2-5\r\n\r\n");
        let partial = String::from_utf8(partial).unwrap();
        assert!(partial.starts_with("HTTP/1.1 206 Partial Content\r\n"));
        assert!(partial.contains("Content-Range: bytes 2-5/10\r\n"));
        assert!(partial.ends_with("\r\n\r\ncdef"));
        let suffix = request(&server, "GET /theme.mp3 HTTP/1.1\r\nRange: bytes=-3\r\n\r\n");
        assert!(suffix.ends_with(b"\r\n\r\nhij"));
        let empty = request(&server, "GET /theme.mp3 HTTP/1.1\r\nRange: bytes=10-\r\n\r\n");
        assert!(empty.starts_with(b"HTTP/1.1 416 Range Not Satisfiable\r\n"));
        let head = request(&server, "HEAD /theme.mp3 HTTP/1.1\r\n\r\n");
        let head = String::from_utf8(head).unwrap();
        assert!(head.contains("Content-Length: 10\r\n"));
        assert!(head.ends_with("\r\n\r\n"));
        let missing = request(&server, "GET /missing.png HTTP/1.1\r\n\r\n");
        assert!(missing.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
        let escaped = request(&server, "GET /%2e%2e/outside HTTP/1.1\r\n\r\n");
        assert!(escaped.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
        let mutation = request(&server, "POST /game.wasm HTTP/1.1\r\n\r\n");
        assert!(mutation.starts_with(b"HTTP/1.1 405 Method Not Allowed\r\n"));
    }
}
