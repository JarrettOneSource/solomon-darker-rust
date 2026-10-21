//! Current Source supervisor close keeps the actual authorities alive for socket
//! release callbacks. Transport shutdown must follow those callbacks, not erase them.

use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use super::HostedSupervisor;
use crate::host::authority::close_peer;
use crate::transport::{HostedHttpResponse, NativeServer, PeerId, TransportClose, TransportEvent};

impl HostedSupervisor {
    pub(super) fn disconnected(
        &mut self,
        peer_id: PeerId,
        close: TransportClose,
        now_ms: f64,
    ) -> Result<(), String> {
        if let Some(restart) = &mut self.deployment {
            // A close releases a wait, but does not acknowledge an uncommitted save.
            restart.pending.remove(&peer_id);
        }
        if let Some(college) =
            self.colleges.iter_mut().find(|college| college.host.connections.contains_key(&peer_id))
        {
            let connection = college.host.connections.remove(&peer_id).unwrap();
            if let Some(social) = college.host.disconnected(connection, now_ms, close)? {
                self.broker.close_connection(&social);
            }
        }
        Ok(())
    }

    pub(super) fn shutdown_authorities(
        &mut self,
        server: &NativeServer,
        started: Instant,
        failed: bool,
    ) -> Result<(), String> {
        server.begin_hosted_deployment();
        // Source closes the private hosts before the persistent Hub. Their socket
        // workers already own a bounded close handshake; no new timeout is needed.
        for college in self.colleges.iter_mut().skip(1) {
            college.host.begin_shutdown(failed)?;
        }
        self.drain_shutdown_connections(server, started, true)?;
        for index in (1..self.colleges.len()).rev() {
            self.close_college(index, server, "supervisor-shutdown")?;
        }
        if let Some(hub) = self.colleges.first_mut() {
            hub.host.begin_shutdown(failed)?;
        }
        self.drain_shutdown_connections(server, started, false)
    }

    fn drain_shutdown_connections(
        &mut self,
        server: &NativeServer,
        started: Instant,
        private_only: bool,
    ) -> Result<(), String> {
        while self
            .colleges
            .iter()
            .skip(usize::from(private_only))
            .any(|college| !college.host.connections.is_empty())
        {
            match server.recv_timeout(Duration::from_secs(1)) {
                Ok(TransportEvent::Disconnected { peer_id, close }) => {
                    self.disconnected(peer_id, close, started.elapsed().as_secs_f64() * 1000.0)?;
                    self.deliver_broker()?;
                }
                Ok(TransportEvent::Connected { peer, .. }) => {
                    close_peer(&peer, 1012, "server shutdown")?;
                }
                Ok(TransportEvent::Http { respond, .. }) => {
                    let _ = respond.send(HostedHttpResponse::empty(503));
                }
                Ok(TransportEvent::Message { .. }) | Err(RecvTimeoutError::Timeout) => {}
                Ok(TransportEvent::Failed { reason }) => return Err(reason),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("The game transport closed before its authorities released".into());
                }
            }
        }
        Ok(())
    }
}
