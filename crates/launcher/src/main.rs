//! Native entry point for the packaged game. The browser is the installed rendering engine;
//! all assets come from this process and all game logic is compiled Rust.

mod desktop_main;
mod supervisor_main;

use std::env;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use solomon_launcher::AssetServer;
use solomon_launcher::host::{NativeAuthority, NativeHostConfig};
use solomon_launcher::transport::{NativeServer, TransportConfig};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Could not start Solomon Darker: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--desktop") {
        return desktop_main::run(arguments);
    }
    if arguments.iter().any(|argument| argument == "--supervisor") {
        return supervisor_main::run(arguments);
    }
    let mut assets = None;
    let mut checkpoint = None;
    let mut port = 4188;
    let mut listen = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut open = true;
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--assets") => {
                assets = Some(PathBuf::from(arguments.next().ok_or("--assets needs a directory")?));
            }
            Some("--save") => {
                checkpoint =
                    Some(PathBuf::from(arguments.next().ok_or("--save needs a file path")?));
            }
            Some("--port") => {
                port = arguments
                    .next()
                    .and_then(|value| value.to_str().and_then(|value| value.parse::<u16>().ok()))
                    .filter(|value| *value != 0)
                    .ok_or("--port needs a number from 1 to 65535")?;
            }
            Some("--listen") => {
                listen = arguments
                    .next()
                    .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
                    .ok_or("--listen needs an IPv4 or IPv6 address")?;
            }
            Some("--no-open") => open = false,
            Some("--help" | "-h") => {
                println!(
                    "Solomon Darker\n\n\
                     Usage: solomon-darker [--assets DIRECTORY] [--port 4188] \
                     [--listen 127.0.0.1] [--save FILE] [--no-open]\n\n\
                     Keep this process running while playing. Press Ctrl+C to quit.\n\
                     This process owns the shared game and durable save.\n\
                     Use the same port and browser origin to resume its saved wizard."
                );
                return Ok(());
            }
            _ => {
                return Err(format!(
                    "unknown argument: {} (use --help)",
                    argument.to_string_lossy()
                ));
            }
        }
    }
    let assets = match assets {
        Some(path) => path,
        None => env::current_exe()
            .map_err(|error| format!("cannot locate the launcher: {error}"))?
            .parent()
            .ok_or("the launcher has no parent directory")?
            .join("game"),
    };
    let server = AssetServer::new(&assets)
        .map_err(|error| format!("cannot load {}: {error}", assets.display()))?;
    // Changing ports silently would change the browser's IndexedDB origin and hide saves.
    let listener = TcpListener::bind((listen, port)).map_err(|error| {
        format!(
            "cannot listen on port {port}: {error}. Close another game launcher using this port"
        )
    })?;
    // An unspecified bind address is useful for direct join, but is not a browser target.
    let local_address = match listen {
        IpAddr::V4(address) if address.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(address) if address.is_unspecified() => {
            IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        }
        address => address,
    };
    let checkpoint = match checkpoint {
        Some(path) => path,
        None => default_checkpoint_path(port)?,
    };
    let authority = NativeAuthority::load(NativeHostConfig::new(assets, checkpoint))?;
    let transport = NativeServer::start(
        listener,
        server,
        TransportConfig {
            // Pending title probes and a replacement socket need space beside sixteen
            // admitted wizards. The authority independently enforces player capacity.
            max_peers: 32,
            ..TransportConfig::default()
        },
    )
    .map_err(|error| format!("cannot start the native game transport: {error}"))?;
    let stop = Arc::new(AtomicBool::new(false));
    let shutdown = stop.clone();
    ctrlc::set_handler(move || shutdown.store(true, Ordering::Release))
        .map_err(|error| format!("cannot install the native shutdown handler: {error}"))?;
    // The marker selects real same-origin authority in the bundled Rust client.
    // An ordinary static preview remains intentional standalone browser mode.
    let url = format!("http://{}/?host=1", std::net::SocketAddr::new(local_address, port));
    println!("Solomon Darker is running at {url}\nKeep this window open. Press Ctrl+C to quit.");
    if open && let Err(error) = open_browser(&url) {
        eprintln!("Could not open the browser: {error}. Open {url} to play.");
    }
    authority.run(transport, stop)
}

fn default_checkpoint_path(port: u16) -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    let directory =
        env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"));
    #[cfg(target_os = "windows")]
    let directory = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let directory = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let directory = directory.ok_or("cannot locate native application data; use --save FILE")?;
    Ok(directory.join("Solomon Darker").join(format!("host-{port}.json")))
}

fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let status = Command::new("/usr/bin/open").arg(url).status();
    #[cfg(target_os = "windows")]
    let status = Command::new("cmd").args(["/C", "start", "", url]).status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let status = Command::new("xdg-open").arg(url).status();
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("the browser opener exited with {status}")),
        Err(error) => Err(error.to_string()),
    }
}
