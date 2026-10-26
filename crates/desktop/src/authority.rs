//! The existing standalone supervisor runs in one application-owned thread.
//! Its route is ready before admission; the native client retains source-save ownership.

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::JoinHandle;
use std::time::Duration;

use base64::Engine;
use solomon_launcher::{
    host::supervisor::{HostedSupervisor, HostedSupervisorConfig},
    transport::{NativeServer, TransportConfig},
};
use solomon_web::desktop::DesktopEndpoint;

pub struct LocalAuthority {
    endpoint: DesktopEndpoint,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<(), String>>>,
}

impl LocalAuthority {
    pub fn start(assets: PathBuf, revision: String) -> Result<Self, String> {
        let mut secret = [0_u8; 32];
        getrandom::fill(&mut secret)
            .map_err(|error| format!("Could not create the game credential: {error}"))?;
        let credential = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);
        let config = HostedSupervisorConfig::new(assets, credential.clone(), revision);
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("Could not bind the local game: {error}"))?;
        let address = listener.local_addr().map_err(|error| error.to_string())?;
        let server = NativeServer::start_hosted(
            listener,
            TransportConfig {
                allowed_origins: vec![solomon_web::platform::NATIVE_ORIGIN.into()],
                max_peers: 32,
                ..TransportConfig::default()
            },
        )
        .map_err(|error| format!("Could not start the local game: {error}"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let (ready, receiver) = mpsc::channel();
        let host_credential = credential.clone();
        let worker = std::thread::Builder::new()
            .name("game-authority".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                let host = HostedSupervisor::new_standalone(config, host_credential)?;
                host.run_when_ready(server, stopping, |_| {
                    let _ = ready.send(());
                })
            })
            .map_err(|error| format!("Could not start the local game: {error}"))?;
        let mut authority = Self {
            endpoint: DesktopEndpoint { url: format!("ws://{address}/game"), credential },
            stop,
            worker: Some(worker),
        };
        match receiver.recv_timeout(Duration::from_secs(30)) {
            Ok(()) => Ok(authority),
            Err(error) => {
                authority.close()?;
                Err(format!("The local game did not become ready: {error}"))
            }
        }
    }

    pub fn endpoint(&self) -> DesktopEndpoint {
        self.endpoint.clone()
    }

    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub fn close(&mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| "The local game authority panicked")??;
        }
        Ok(())
    }
}

impl Drop for LocalAuthority {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            solomon_web::platform::error(&error);
        }
    }
}
