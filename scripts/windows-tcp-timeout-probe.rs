//! Diagnostic only: does a real socket remain usable after the launcher's idle read timeout?
//! The blocking case uses TransportConfig's 10 ms read and 250 ms write deadlines.
//! The nonblocking case is a discriminator, not a proposed replacement transport.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
enum Mode {
    BlockingTimeout,
    NonblockingControl,
}

#[derive(Debug, Default)]
struct Observation {
    idle_read_errors: usize,
    first_read_error_kind: Option<io::ErrorKind>,
    first_read_raw_os_error: Option<i32>,
    first_read_error_after_ms: Option<u128>,
    outbound_ticks: usize,
    received_bytes: usize,
}

fn server_exchange(
    stream: &mut TcpStream,
    mode: Mode,
    observed: &mut Observation,
) -> Result<(), String> {
    let started = Instant::now();
    let mut last_tick = started;
    let mut received = [0; 5];
    while observed.received_bytes < received.len() {
        if started.elapsed() >= Duration::from_secs(5) {
            return Err("server exchange exceeded its five-second deadline".into());
        }
        if last_tick.elapsed() >= Duration::from_millis(50) {
            stream
                .write_all(b"T")
                .map_err(|error| format!("write-after-idle: {error:?}"))?;
            observed.outbound_ticks += 1;
            last_tick = Instant::now();
        }
        match stream.read(&mut received[observed.received_bytes..]) {
            Ok(0) => return Err("read: unexpected EOF before delayed input".into()),
            Ok(length) => observed.received_bytes += length,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                if observed.idle_read_errors == 0 {
                    observed.first_read_error_kind = Some(error.kind());
                    observed.first_read_raw_os_error = error.raw_os_error();
                    observed.first_read_error_after_ms = Some(started.elapsed().as_millis());
                }
                observed.idle_read_errors += 1;
            }
            Err(error) => return Err(format!("read-after-idle: {error:?}")),
        }
        // This is the actual launcher's post-read flush boundary.
        stream
            .flush()
            .map_err(|error| format!("flush-after-read: {error:?}"))?;
        if matches!(mode, Mode::NonblockingControl) {
            thread::sleep(Duration::from_millis(10));
        }
    }
    if received != *b"input" {
        return Err(format!("delayed input changed: {received:?}"));
    }
    stream
        .write_all(b"E")
        .map_err(|error| format!("final-reply: {error:?}"))?;
    Ok(())
}

fn probe(mode: Mode, attempt: usize) -> io::Result<bool> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let mut client = TcpStream::connect(listener.local_addr()?)?;
    let (mut server, _) = listener.accept()?;
    server.set_nodelay(true)?;
    server.set_write_timeout(Some(Duration::from_millis(250)))?;
    match mode {
        Mode::BlockingTimeout => server.set_read_timeout(Some(Duration::from_millis(10)))?,
        Mode::NonblockingControl => server.set_nonblocking(true)?,
    }
    client.set_nodelay(true)?;
    client.set_read_timeout(Some(Duration::from_secs(2)))?;
    client.set_write_timeout(Some(Duration::from_secs(2)))?;
    let client_worker = thread::spawn(move || -> io::Result<usize> {
        // Span many production 10 ms read deadlines before delivering actual data.
        thread::sleep(Duration::from_millis(500));
        client.write_all(b"input")?;
        let mut ticks = 0;
        loop {
            let mut byte = [0];
            client.read_exact(&mut byte)?;
            match byte[0] {
                b'T' => ticks += 1,
                b'E' => return Ok(ticks),
                other => return Err(io::Error::other(format!("unexpected reply byte: {other}"))),
            }
        }
    });
    let started = Instant::now();
    let mut observed = Observation::default();
    let server_result = server_exchange(&mut server, mode, &mut observed);
    if server_result.is_err() {
        let _ = server.shutdown(Shutdown::Both);
    }
    let client_result = client_worker
        .join()
        .map_err(|_| io::Error::other("client worker panicked"))?;
    let passed = server_result.is_ok()
        && client_result
            .as_ref()
            .is_ok_and(|ticks| *ticks == observed.outbound_ticks)
        && observed.idle_read_errors > 0;
    println!(
        "mode={mode:?} attempt={attempt} passed={passed} elapsed_ms={} server={server_result:?} client={client_result:?} observed={observed:?}",
        started.elapsed().as_millis(),
    );
    Ok(passed)
}

fn main() -> io::Result<()> {
    println!(
        "os={} arch={}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let mut passed = true;
    for attempt in 1..=3 {
        for mode in [Mode::BlockingTimeout, Mode::NonblockingControl] {
            passed &= probe(mode, attempt)?;
        }
    }
    if !passed {
        return Err(io::Error::other(
            "at least one socket exchange failed; inspect the per-stage observations",
        ));
    }
    Ok(())
}
