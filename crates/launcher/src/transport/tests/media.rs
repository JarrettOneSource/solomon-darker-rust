//! Paused HTML media uses the real public native HTTP listener and socket lifetime.

use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::process::Command;

use sha2::{Digest, Sha256};

use super::*;

fn media_assets() -> TestAssets {
    let assets = TestAssets::new();
    let mut file = fs::File::create(assets.0.join("paused.mp3")).unwrap();
    let block: Vec<u8> = (0..64 * 1024).map(|index| (index % 251) as u8).collect();
    for _ in 0..1024 {
        file.write_all(&block).unwrap();
    }
    assets
}

fn media_request(server: &NativeServer, range: &str) -> (BufReader<TcpStream>, String) {
    let mut stream = TcpStream::connect(server.local_addr()).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    write!(stream, "GET /paused.mp3 HTTP/1.1\r\nHost: localhost\r\n{range}\r\n").unwrap();
    let mut reader = BufReader::new(stream);
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0, "the response includes complete headers");
        headers.push_str(&line);
        if line == "\r\n" {
            return (reader, headers);
        }
    }
}

fn body_hash(reader: &mut impl Read) -> (u64, Vec<u8>) {
    let mut bytes = [0; 64 * 1024];
    let mut count = 0;
    let mut hash = Sha256::new();
    loop {
        let read = reader.read(&mut bytes).unwrap();
        if read == 0 {
            return (count, hash.finalize().to_vec());
        }
        count += read as u64;
        hash.update(&bytes[..read]);
    }
}

#[test]
fn asset_http_paused_range_finishes_after_the_browser_resumes() {
    let assets = media_assets();
    let mut server = assets.start(TransportConfig::default());
    // Python is already a build/CI prerequisite and exposes the native portable
    // receive-window option that std::net lacks. No server test knob is involved.
    let python = std::env::var_os("PYTHON")
        .unwrap_or_else(|| if cfg!(windows) { "python".into() } else { "python3".into() });
    let output = Command::new(python)
        .args([
            "-c",
            r#"
import hashlib, json, socket, sys, time
with socket.socket() as stream:
    stream.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 65536)
    receive_buffer = stream.getsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF)
    stream.settimeout(3)
    stream.connect(('127.0.0.1', int(sys.argv[1])))
    stream.sendall(b'GET /paused.mp3 HTTP/1.1\r\nHost: localhost\r\nRange: bytes=17-18799992\r\n\r\n')
    with stream.makefile('rb') as reader:
        headers = b''
        while not headers.endswith(b'\r\n\r\n'):
            line = reader.readline()
            if not line:
                raise RuntimeError('incomplete response headers')
            headers += line
        time.sleep(40)
        count, digest = 0, hashlib.sha256()
        while chunk := reader.read(65536):
            count += len(chunk)
            digest.update(chunk)
        print(json.dumps({'headers': headers.decode('ascii'), 'count': count,
                          'digest': list(digest.digest()), 'receive_buffer': receive_buffer}))
"#,
            &server.local_addr().port().to_string(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "TCP client failed: {}", String::from_utf8_lossy(&output.stderr));
    #[derive(serde::Deserialize)]
    struct Received {
        headers: String,
        count: u64,
        digest: Vec<u8>,
        receive_buffer: usize,
    }
    let received: Received = serde_json::from_slice(&output.stdout).unwrap();
    assert!(received.receive_buffer <= 256 * 1024, "the actual TCP window stays bounded");
    let count = 18_799_976;
    assert!(received.headers.starts_with("HTTP/1.1 206 Partial Content\r\n"));
    assert!(received.headers.contains("Content-Type: audio/mpeg\r\n"));
    assert!(received.headers.contains(&format!("Content-Length: {count}\r\n")));
    assert!(received.headers.contains("Content-Range: bytes 17-18799992/67108864\r\n"));
    assert_eq!(received.count, count, "paused buffering cannot truncate the declared206 body");
    let mut file = fs::File::open(assets.0.join("paused.mp3")).unwrap();
    file.seek(SeekFrom::Start(17)).unwrap();
    let expected = body_hash(&mut file.take(count));
    assert_eq!((received.count, received.digest), expected, "resumed range bytes retain their complete source digest");
    server.shutdown();
}

#[test]
fn asset_http_paused_readers_leave_framing_available_and_shutdown_is_prompt() {
    let assets = media_assets();
    let mut server = assets.start(TransportConfig::default());
    let mut paused = Vec::new();
    for _ in 0..HTTP_WORKERS {
        let (reader, headers) = media_request(&server, "");
        assert!(headers.starts_with("HTTP/1.1 200 OK\r\n"));
        paused.push(reader);
    }
    let mut small = TcpStream::connect(server.local_addr()).unwrap();
    small.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    small.write_all(b"GET /game.wasm HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    let mut response = Vec::new();
    small.read_to_end(&mut response).expect("paused media cannot occupy the framing workers");
    assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert!(response.ends_with(b"\0asm0123456789"));
    let stopped = Instant::now();
    server.shutdown();
    assert!(stopped.elapsed() < Duration::from_secs(2), "shutdown wakes owned asset writers");
    drop(paused);
}
