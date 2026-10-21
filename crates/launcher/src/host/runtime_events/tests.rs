use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Instant;

const SECRET: &str = "runtime-event-fixture-secret-000000000000";

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../sim/tests/fixtures/website/runtime-events-goldens.json"
    ))
    .unwrap()
}

fn entry(id: u64) -> RuntimeEventEntry {
    RuntimeEventEntry {
        component: "game-host".into(),
        event: "fixture.accepted".into(),
        message: "\u{2728} Accepted".into(),
        occurred_at_utc: Some("2026-10-21T12:00:00.000Z".into()),
        details: [
            ("id".into(), LuaValue::Number(id as f64)),
            ("value".into(), LuaValue::Number(0.25)),
        ]
        .into_iter()
        .collect(),
    }
}

#[test]
fn runtime_event_admission_matches_source_whatwg_urls_and_utf16_secrets() {
    for case in fixture()["configuration"].as_array().unwrap() {
        let result = RuntimeEventConfig::new(
            case["endpoint"].as_str().unwrap(),
            case["secret"].as_str().unwrap().into(),
        );
        if case["accepted"].as_bool().unwrap() {
            assert_eq!(result.unwrap().endpoint.as_str(), case["normalized"].as_str().unwrap());
        } else {
            assert_eq!(result.err().unwrap(), case["error"].as_str().unwrap());
        }
    }
}

#[test]
fn actual_runtime_event_http_retries_and_close_replay_the_source() {
    let fixture = fixture();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().take(2).enumerate() {
        let endpoint = Endpoint::new(if index == 0 { 2 } else { usize::MAX }, None);
        let mut publisher = RuntimeEventPublisher::new(
            RuntimeEventConfig::new(&endpoint.url, SECRET.into()).unwrap(),
        )
        .unwrap();
        let sink = publisher.sink();
        for id in case["entries"].as_array().unwrap() {
            sink.publish(entry(id.as_u64().unwrap()));
        }
        publisher.close();
        sink.publish(entry(99));
        assert_eq!(
            *endpoint.requests.lock().unwrap(),
            *case["requests"].as_array().unwrap(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn runtime_event_overflow_preserves_sources_in_flight_shift_semantics() {
    let (release, held) = mpsc::channel();
    let endpoint = Endpoint::new(0, Some(held));
    let mut publisher =
        RuntimeEventPublisher::new(RuntimeEventConfig::new(&endpoint.url, SECRET.into()).unwrap())
            .unwrap();
    let sink = publisher.sink();
    sink.publish(entry(0));
    let deadline = Instant::now() + Duration::from_secs(5);
    while endpoint.requests.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "the endpoint receives the actual first request");
        thread::sleep(Duration::from_millis(1));
    }
    for id in 1..=260 {
        sink.publish(entry(id));
    }
    release.send(()).unwrap();
    publisher.close();
    let ids: Vec<_> = endpoint
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|request| request["document"]["details"]["id"].clone())
        .collect();
    assert_eq!(ids, *fixture()["cases"][2]["requestIds"].as_array().unwrap());
}

/// A real loopback endpoint; Source's native HTTP recorder uses the same response
/// schedules. It holds the first response only for the explicit overflow case.
struct Endpoint {
    url: String,
    requests: Arc<Mutex<Vec<serde_json::Value>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Endpoint {
    fn new(failures: usize, mut held: Option<mpsc::Receiver<()>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&requests);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                let (stream, _) = match listener.accept() {
                    Ok(accepted) => accepted,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => panic!("runtime-event endpoint accept: {error}"),
                };
                // Darwin inherits O_NONBLOCK from the listener. A read timeout
                // does not clear it; accept can run before the client writes.
                stream.set_nonblocking(false).unwrap();
                let request = read_request(&stream);
                let count = {
                    let mut requests = received.lock().unwrap();
                    requests.push(request);
                    requests.len()
                };
                if let Some(held) = held.take() {
                    held.recv_timeout(Duration::from_secs(5)).unwrap();
                }
                let status = if count <= failures { "503 Service Unavailable" } else { "200 OK" };
                write!(
                    &stream,
                    "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
            }
        });
        Self {
            url: format!("http://{address}/runtime-events?source=rust"),
            requests,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let result = self.worker.take().unwrap().join();
        // Preserve an existing test failure instead of aborting the whole binary
        // with a second panic while its endpoint worker is being reaped.
        if !thread::panicking() {
            result.unwrap();
        }
    }
}

fn read_request(stream: &TcpStream) -> serde_json::Value {
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let start: Vec<_> = line.split_whitespace().collect();
    let method = start[0].to_owned();
    let path = start[1].to_owned();
    let mut headers = std::collections::BTreeMap::new();
    loop {
        line.clear();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        let (name, value) = line.split_once(':').unwrap();
        headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
    }
    let length: usize = headers["content-length"].parse().unwrap();
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).unwrap();
    let body = String::from_utf8(bytes).unwrap();
    let document: serde_json::Value = serde_json::from_str(&body).unwrap();
    serde_json::json!({ "path":path, "method":method, "authorization":headers["authorization"],
        "contentType":headers["content-type"], "body":body, "document":document })
}
