//! Native ownership of the published encrypted desktop peer transport.
//! The authority stays on loopback. Only explicitly starting Host exposes TLS.

mod invite;
mod tls;
pub use invite::Invite;

use std::net::{Ipv4Addr, TcpListener as StdListener};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinSet,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

enum Mode {
    Host { authority_port: u16, invite: Invite },
    Join(Invite),
}

pub struct Peer {
    port: u16,
    stop: watch::Sender<bool>,
    worker: Option<JoinHandle<Result<(), String>>>,
}

impl Peer {
    pub fn host(
        authority_port: u16,
        port: u16,
        revision: &str,
        secret: &str,
    ) -> Result<Self, String> {
        let listener = StdListener::bind((Ipv4Addr::UNSPECIFIED, port))
            .map_err(|error| format!("Could not listen for friends: {error}"))?;
        let port = listener.local_addr().map_err(|error| error.to_string())?.port();
        let invite = Invite::new("localhost", port, revision, secret)?;
        Self::start(listener, Mode::Host { authority_port, invite })
    }

    /// The initial authenticated probe completes before a join can be admitted.
    pub fn join(invite: Invite) -> Result<Self, String> {
        let listener = StdListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("Could not open the local peer connection: {error}"))?;
        Self::start(listener, Mode::Join(invite))
    }

    fn start(listener: StdListener, mode: Mode) -> Result<Self, String> {
        let port = listener.local_addr().map_err(|error| error.to_string())?.port();
        listener.set_nonblocking(true).map_err(|error| error.to_string())?;
        let (stop, stopping) = watch::channel(false);
        let (ready, receiver) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("game-peer".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| format!("Peer runtime: {error}"))?;
                let result = runtime.block_on(serve(listener, mode, stopping, ready));
                // Cancellation also retires DNS work without delaying window shutdown.
                runtime.shutdown_background();
                result
            })
            .map_err(|error| format!("Could not start the peer connection: {error}"))?;
        let mut peer = Self { port, stop, worker: Some(worker) };
        match receiver.recv_timeout(CONNECT_TIMEOUT + Duration::from_secs(2)) {
            Ok(Ok(())) => Ok(peer),
            result => {
                let shutdown = peer.close();
                Err(match result {
                    Ok(Err(error)) => error,
                    Err(error) => {
                        shutdown.err().unwrap_or_else(|| format!("Peer startup: {error}"))
                    }
                    Ok(Ok(())) => unreachable!(),
                })
            }
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub fn close(&mut self) -> Result<(), String> {
        let _ = self.stop.send(true);
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| "The peer connection panicked")??;
        }
        Ok(())
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            solomon_web::platform::error(&error);
        }
    }
}

async fn serve(
    listener: StdListener,
    mode: Mode,
    mut stopping: watch::Receiver<bool>,
    ready: mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    let setup = async {
        let listener = TcpListener::from_std(listener).map_err(|error| error.to_string())?;
        let context = match &mode {
            Mode::Host { invite, .. } => tls::host(invite)?,
            Mode::Join(invite) => {
                let context = tls::guest(invite)?;
                let probe = tls::connect(invite, &context).await.map_err(|_| {
                    "Could not connect to the host. Check the invite, firewall and forwarded TCP port."
                })?;
                drop(probe);
                context
            }
        };
        Ok::<_, String>((listener, context))
    };
    let result = tokio::select! {
        _ = stopping.changed() => return Ok(()),
        result = setup => result,
    };
    let (listener, context) = match result {
        Ok(ready_state) => {
            let _ = ready.send(Ok(()));
            ready_state
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return Ok(());
        }
    };
    let limit = match &mode {
        Mode::Host { .. } => 32,
        Mode::Join(_) => 16,
    };
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _ = stopping.changed() => break,
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    return Err(format!("The peer connection task stopped: {error}"));
                }
            }
            accepted = listener.accept() => {
                let (mut local, _) = accepted.map_err(|error| format!("Peer listener: {error}"))?;
                if connections.len() >= limit { continue; }
                let context = context.clone();
                match &mode {
                    Mode::Host { authority_port, .. } => {
                        let authority_port = *authority_port;
                        connections.spawn(async move {
                            let mut peer = tls::accept(local, &context).await?;
                            let mut authority = tokio::time::timeout(CONNECT_TIMEOUT,
                                TcpStream::connect((Ipv4Addr::LOCALHOST, authority_port))).await
                                .map_err(|_| "Local authority connection timed out".to_owned())?
                                .map_err(|error| error.to_string())?;
                            authority.set_nodelay(true).map_err(|error| error.to_string())?;
                            copy_bidirectional(&mut peer, &mut authority).await.map_err(|error| error.to_string())?;
                            Ok::<_, String>(())
                        });
                    }
                    Mode::Join(invite) => {
                        let invite = invite.clone();
                        connections.spawn(async move {
                            let mut peer = tls::connect(&invite, &context).await?;
                            local.set_nodelay(true).map_err(|error| error.to_string())?;
                            copy_bidirectional(&mut local, &mut peer).await.map_err(|error| error.to_string())?;
                            Ok::<_, String>(())
                        });
                    }
                }
            }
        }
    }
    connections.abort_all();
    while connections.join_next().await.is_some() {}
    Ok(())
}

#[cfg(test)]
mod tests;
