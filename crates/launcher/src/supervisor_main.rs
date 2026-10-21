//! Source-compatible hosted process configuration. Secret values are read from the
//! existing supervisor environment; command output contains only the bound address.

use std::env;
use std::ffi::OsString;
use std::net::{IpAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use solomon_launcher::host::supervisor::{HostedSupervisor, HostedSupervisorConfig};
use solomon_launcher::transport::{NativeServer, TransportConfig};

pub fn run(arguments: Vec<OsString>) -> Result<(), String> {
    let mut assets = env::var_os("SDR_GAME_ASSET_DIRECTORY").map(PathBuf::from);
    let mut port = integer("SDR_GAME_SUPERVISOR_PORT", 5222, 0, 65535)? as u16;
    let mut host: IpAddr = env::var("SDR_GAME_SUPERVISOR_HOST")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse()
        .map_err(|_| "SDR_GAME_SUPERVISOR_HOST must be an IP address")?;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--supervisor" | "--no-open") => {}
            Some("--assets") => {
                assets = Some(PathBuf::from(arguments.next().ok_or("--assets needs a directory")?))
            }
            Some("--port") => {
                port = arguments
                    .next()
                    .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
                    .ok_or("--port needs a TCP port")?
            }
            Some("--listen") => {
                host = arguments
                    .next()
                    .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
                    .ok_or("--listen needs an IP address")?
            }
            _ => return Err(format!("unknown hosted argument: {}", argument.to_string_lossy())),
        }
    }
    if !host.is_loopback() {
        return Err(
            "The game session supervisor must bind to loopback behind the TLS gateway".into()
        );
    }
    let assets =
        assets.ok_or("Set SDR_GAME_ASSET_DIRECTORY or pass --assets for hosted navigation")?;
    let secret = required("SDR_GAME_SUPERVISOR_SECRET")?;
    let revision = required("SDR_GAME_REVISION")?.trim().to_lowercase();
    let origins: Vec<_> = required("SDR_GAME_ALLOWED_ORIGINS")?
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    if origins.is_empty() {
        return Err("SDR_GAME_ALLOWED_ORIGINS must contain at least one browser origin".into());
    }
    let mut config = HostedSupervisorConfig::new(assets, secret, revision);
    // Source `run-game-session-supervisor.ts:48` requires this same durable path.
    config.memorial_path = Some(PathBuf::from(required("SDR_GAME_MEMORIAL_PATH")?));
    let event_endpoint = env::var("SDR_RUNTIME_EVENT_ENDPOINT").unwrap_or_default();
    let event_secret = env::var("SDR_RUNTIME_EVENT_SECRET").unwrap_or_default();
    if event_endpoint.trim().is_empty() != event_secret.trim().is_empty() {
        return Err(
            "SDR_RUNTIME_EVENT_ENDPOINT and SDR_RUNTIME_EVENT_SECRET must be configured together"
                .into(),
        );
    }
    if !event_endpoint.trim().is_empty() {
        config.runtime_events =
            Some(solomon_launcher::host::runtime_events::RuntimeEventConfig::new(
                event_endpoint.trim(),
                event_secret.trim().to_owned(),
            )?);
    }
    let checkpoint =
        env::var_os("SDR_GAME_ML_BOT_CHECKPOINT").map(PathBuf::from).unwrap_or_else(|| {
            config.asset_directory.join("assets/native/ml-bot-policy-v7-selected.sdml")
        });
    let bytes = std::fs::read(&checkpoint)
        .map_err(|error| format!("Cannot read ML bot policy {}: {error}", checkpoint.display()))?;
    let policy = solomon_assets::ml_bot_checkpoint::decode_ml_bot_policy_checkpoint(&bytes)?;
    config.ml_bot_policy = Some(Arc::new(policy.into_runtime()?));
    config.maximum_players = integer("SDR_GAME_MAX_CONNECTIONS_PER_SESSION", 16, 1, 10_000)?;
    config.maximum_sessions = integer("SDR_GAME_MAX_SESSIONS", 64, 1, 10_000)?;
    config.unclaimed_timeout =
        Duration::from_secs(integer("SDR_GAME_UNCLAIMED_TIMEOUT_SECONDS", 120, 1, 86_400)? as u64);
    config.deployment_save_timeout = Duration::from_secs(integer(
        "SDR_GAME_DEPLOYMENT_SAVE_TIMEOUT_SECONDS",
        30,
        1,
        300,
    )? as u64);
    let capacity = config
        .maximum_players
        .checked_mul(config.maximum_sessions + 1)
        .ok_or("Supervisor connection capacity overflow")?;
    let supervisor = HostedSupervisor::new(config)?;
    let server = NativeServer::start_hosted(
        TcpListener::bind((host, port)).map_err(|error| error.to_string())?,
        TransportConfig {
            allowed_origins: origins,
            max_peers: capacity,
            ..TransportConfig::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let address = server.local_addr();
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Release))
        .map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::json!({"type":"ready","url":format!("http://{address}"),"protocol":"solomon-game-v1"})
    );
    supervisor.run(server, stop)
}

fn required(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} must be configured"))
}
fn integer(name: &str, fallback: usize, minimum: usize, maximum: usize) -> Result<usize, String> {
    match env::var(name).ok().filter(|value| !value.trim().is_empty()) {
        None => Ok(fallback),
        Some(value) => value
            .parse()
            .ok()
            .filter(|value| *value >= minimum && *value <= maximum)
            .ok_or_else(|| format!("{name} must be an integer within {minimum}..{maximum}")),
    }
}
