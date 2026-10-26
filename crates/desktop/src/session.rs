//! One native desktop session owns its authority and optional encrypted peer service.

use solomon_web::desktop::{DesktopEndpoint, launcher::SessionOptions};
use std::path::PathBuf;

use crate::{
    authority::LocalAuthority,
    peer::{Invite, Peer},
};

pub struct Session {
    endpoint: DesktopEndpoint,
    pub options: SessionOptions,
    pub invite: Option<String>,
    peer: Option<Peer>,
    authority: Option<LocalAuthority>,
}

impl Session {
    pub fn start(options: SessionOptions, assets: PathBuf, revision: &str) -> Result<Self, String> {
        match &options {
            SessionOptions::Join { invite } => {
                let invite = Invite::parse(invite, revision)?;
                let peer = Peer::join(invite.clone())?;
                Ok(Self {
                    endpoint: DesktopEndpoint {
                        url: format!("ws://127.0.0.1:{}/game", peer.port()),
                        credential: invite.secret,
                    },
                    options,
                    invite: None,
                    peer: Some(peer),
                    authority: None,
                })
            }
            SessionOptions::Solo | SessionOptions::Host { .. } => {
                let authority = LocalAuthority::start(assets, revision.to_owned())?;
                let endpoint = authority.endpoint();
                let (peer, invite) = if let SessionOptions::Host { host, port } = &options {
                    let invite = Invite::new(host, *port, revision, &endpoint.credential)?;
                    let authority_port = url::Url::parse(&endpoint.url)
                        .map_err(|error| error.to_string())?
                        .port()
                        .ok_or("Local authority port missing")?;
                    let peer = Peer::host(authority_port, *port, revision, &endpoint.credential)?;
                    (Some(peer), Some(invite.url()))
                } else {
                    (None, None)
                };
                Ok(Self { endpoint, options, invite, peer, authority: Some(authority) })
            }
        }
    }

    pub fn endpoint(&self) -> DesktopEndpoint {
        self.endpoint.clone()
    }

    pub fn failure(&self) -> Option<&'static str> {
        if self.authority.as_ref().is_some_and(LocalAuthority::is_finished) {
            Some("The local game stopped. Your last saved checkpoint is still on this device.")
        } else if self.peer.as_ref().is_some_and(Peer::is_finished) {
            Some("The peer connection stopped. Return to the launcher to reconnect.")
        } else {
            None
        }
    }

    pub fn close(&mut self) -> Result<(), String> {
        let peer = self.peer.take().map(|mut peer| peer.close()).transpose();
        let authority = self.authority.take().map(|mut authority| authority.close()).transpose();
        peer?;
        authority?;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            solomon_web::platform::error(&error);
        }
    }
}
