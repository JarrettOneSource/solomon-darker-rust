use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpStream as StdStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};

struct Echo {
    port: u16,
    stop: Arc<AtomicBool>,
    accepted: Arc<AtomicUsize>,
    worker: Option<JoinHandle<()>>,
}

impl Echo {
    fn start() -> Self {
        let listener = StdListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let accepted = Arc::new(AtomicUsize::new(0));
        let arrivals = accepted.clone();
        let worker = std::thread::spawn(move || {
            let mut clients = Vec::new();
            while !stopping.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        arrivals.fetch_add(1, Ordering::Relaxed);
                        let stopping = stopping.clone();
                        clients.push(std::thread::spawn(move || {
                            // Accepted sockets inherit nonblocking mode on macOS.
                            stream.set_nonblocking(false).unwrap();
                            stream.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
                            stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                            let mut bytes = [0; 8192];
                            while !stopping.load(Ordering::Acquire) {
                                match stream.read(&mut bytes) {
                                    Ok(0) => break,
                                    Ok(count) => {
                                        if stream.write_all(&bytes[..count]).is_err() {
                                            break;
                                        }
                                    }
                                    Err(error)
                                        if matches!(
                                            error.kind(),
                                            std::io::ErrorKind::WouldBlock
                                                | std::io::ErrorKind::TimedOut
                                        ) => {}
                                    Err(_) => break,
                                }
                            }
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("echo accept: {error}"),
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self { port, stop, accepted, worker: Some(worker) }
    }
}

impl Drop for Echo {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn host(authority_port: u16) -> (Peer, Invite) {
    // Tests expose only loopback; the product binds publicly only after Host is chosen.
    let listener = StdListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let invite =
        Invite::new("127.0.0.1", port, &"a".repeat(40), &URL_SAFE_NO_PAD.encode([17; 32])).unwrap();
    let peer =
        Peer::start(listener, Mode::Host { authority_port, invite: invite.clone() }).unwrap();
    (peer, invite)
}

fn stream(port: u16) -> StdStream {
    let stream = StdStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    stream
}

#[test]
fn published_key_derivation_matches_node_and_the_tls_session_uses_the_same_cipher() {
    // Recorded with Node crypto.hkdfSync from the published peer-network.mjs contract.
    let key = tls::transport_key(&URL_SAFE_NO_PAD.encode([17; 32])).unwrap();
    assert_eq!(
        key.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "dff6f2db025ee687f8475126ace1175d0d1cd84fb06d401e3da1603d329c32c0"
    );
    let echo = Echo::start();
    let (_peer, invite) = host(echo.port);
    let context = tls::guest(&invite).unwrap();
    let ssl = openssl::ssl::Ssl::new(&context).unwrap();
    let mut connected = ssl.connect(stream(invite.port)).unwrap();
    assert_eq!(connected.ssl().version_str(), "TLSv1.3");
    assert_eq!(connected.ssl().current_cipher().unwrap().name(), "TLS_AES_128_GCM_SHA256");
    assert!(connected.ssl().peer_certificate().is_none());
    connected.write_all(b"ordinary application bytes").unwrap();
    let mut result = [0; 26];
    connected.read_exact(&mut result).unwrap();
    assert_eq!(&result, b"ordinary application bytes");
}

fn assert_disconnected(stream: &mut StdStream) {
    match stream.read(&mut [0; 1024]) {
        Ok(0) => {}
        Err(error) => assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ),
            "expected a closed socket, got {error}"
        ),
        result => panic!("expected a closed socket, got {result:?}"),
    }
}

#[test]
fn native_host_and_join_forward_large_bidirectional_streams_and_close_live_clients() {
    let echo = Echo::start();
    let (mut host, invite) = host(echo.port);
    let mut join = Peer::join(invite).unwrap();
    let mut client = stream(join.port());
    let mut writer = client.try_clone().unwrap();
    let bytes: Vec<u8> = (0..2 * 1024 * 1024).map(|index| (index % 251) as u8).collect();
    let sent = bytes.clone();
    let writing = std::thread::spawn(move || writer.write_all(&sent).unwrap());
    let mut received = vec![0; bytes.len()];
    client.read_exact(&mut received).unwrap();
    writing.join().unwrap();
    assert_eq!(received, bytes);
    let started = Instant::now();
    join.close().unwrap();
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_disconnected(&mut client);
    host.close().unwrap();
    assert!(StdStream::connect((Ipv4Addr::LOCALHOST, host.port())).is_err());
}

#[test]
fn wrong_key_identity_and_plaintext_never_reach_the_authority() {
    let echo = Echo::start();
    let (_host, invite) = host(echo.port);
    for changed in [
        Invite::new(&invite.host, invite.port, &invite.revision, &URL_SAFE_NO_PAD.encode([19; 32]))
            .unwrap(),
        Invite::new(&invite.host, invite.port, &"b".repeat(40), &invite.secret).unwrap(),
    ] {
        assert!(Peer::join(changed).is_err());
    }
    let mut plaintext = stream(invite.port);
    plaintext.write_all(b"GET /game HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    assert_disconnected(&mut plaintext);
    assert_eq!(echo.accepted.load(Ordering::Relaxed), 0);
}

#[test]
fn shutdown_cancels_unfinished_handshakes_without_waiting_for_the_deadline() {
    let echo = Echo::start();
    let (mut host, _) = host(echo.port);
    let mut stalled = stream(host.port());
    stalled.write_all(&[0x16, 0x03, 0x03]).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let start = Instant::now();
    host.close().unwrap();
    assert!(start.elapsed() < Duration::from_millis(500));
    assert_disconnected(&mut stalled);
    let _ = stalled.shutdown(Shutdown::Both);
}
