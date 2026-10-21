//! Desktop authority child, replacing Source `host/run-game-host.ts` only.
//! Electron owns the stable origin and source-save files; this process owns one
//! ordinary hosted simulation and exits when its supervising pipe closes.

use std::env;
use std::ffi::OsString;
use std::io;
use std::net::{IpAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use solomon_launcher::host::supervisor::{HostedSupervisor, HostedSupervisorConfig};
use solomon_launcher::transport::{NativeServer, TransportConfig};

pub fn run(arguments: Vec<OsString>) -> Result<(), String> {
    let mut assets = env::var_os("SDR_GAME_ASSET_DIRECTORY").map(PathBuf::from);
    let mut host: IpAddr = env::var("SDR_GAME_HOST")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse()
        .map_err(|_| "SDR_GAME_HOST must be a loopback IP address")?;
    let mut port: u16 = env::var("SDR_GAME_PORT")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .map_err(|_| "SDR_GAME_PORT must be an integer between 0 and 65535")?;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--desktop" | "--no-open") => {}
            Some("--assets") => {
                assets = Some(PathBuf::from(arguments.next().ok_or("--assets needs a directory")?));
            }
            Some("--port") => {
                port = arguments
                    .next()
                    .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
                    .ok_or("--port needs a TCP port from 0 to 65535")?;
            }
            Some("--listen") => {
                host = arguments
                    .next()
                    .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
                    .ok_or("--listen needs a loopback IP address")?;
            }
            _ => return Err(format!("unknown desktop argument: {}", argument.to_string_lossy())),
        }
    }
    if !host.is_loopback() {
        return Err("The desktop authority must bind to loopback".into());
    }
    let assets = assets.ok_or("Set SDR_GAME_ASSET_DIRECTORY or pass --assets for desktop")?;
    let credential = required("SDR_GAME_BOOTSTRAP_CREDENTIAL")?;
    let revision = required("SDR_GAME_REVISION")?;
    let origins: Vec<_> = required("SDR_GAME_ALLOWED_ORIGINS")?
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    if origins.is_empty() {
        return Err("SDR_GAME_ALLOWED_ORIGINS must contain the desktop origin".into());
    }
    let config = HostedSupervisorConfig::new(assets, credential.clone(), revision);
    let supervisor = HostedSupervisor::new_standalone(config, credential)?;
    let server = NativeServer::start_hosted(
        TcpListener::bind((host, port)).map_err(|error| error.to_string())?,
        TransportConfig { allowed_origins: origins, max_peers: 32, ..TransportConfig::default() },
    )
    .map_err(|error| error.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stop);
    ctrlc::set_handler(move || signal.store(true, Ordering::Release))
        .map_err(|error| error.to_string())?;
    let parent_closed = Arc::clone(&stop);
    std::thread::Builder::new()
        .name("desktop-parent-lifetime".into())
        .spawn(move || {
            // A native child has no Node IPC channel. EOF on the pipe provides
            // the same parent-disconnect lifetime even after an abrupt app exit.
            let _ = io::copy(&mut io::stdin().lock(), &mut io::sink());
            parent_closed.store(true, Ordering::Release);
        })
        .map_err(|error| format!("Cannot watch the desktop parent: {error}"))?;
    supervisor.run_when_ready(server, stop, |address| {
        println!(
            "{}",
            serde_json::json!({"type":"ready","url":format!("ws://{address}/game"),"protocol":"solomon-game-v1"})
        );
    })
}

fn required(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} must be configured"))
}
