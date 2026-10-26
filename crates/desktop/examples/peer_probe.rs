//! Pipe-owned transport qualification; credentials enter only through stdin.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    use serde::Deserialize;
    use solomon_desktop::peer::{Invite, Peer};
    use std::io::{BufRead, Write};

    #[derive(Deserialize)]
    #[serde(tag = "mode", rename_all = "lowercase")]
    enum Options {
        Host { authority_port: u16, port: u16, revision: String, secret: String },
        Join { invite: String, revision: String },
    }
    let input = std::io::stdin();
    let mut lines = input.lock().lines();
    let configuration = lines
        .next()
        .ok_or("Missing peer probe configuration")?
        .map_err(|error| error.to_string())?;
    let options = serde_json::from_str(&configuration).map_err(|error| error.to_string())?;
    let mut peer = match options {
        Options::Host { authority_port, port, revision, secret } => {
            Peer::host(authority_port, port, &revision, &secret)?
        }
        Options::Join { invite, revision } => Peer::join(Invite::parse(&invite, &revision)?)?,
    };
    println!("{}", serde_json::json!({"port":peer.port()}));
    std::io::stdout().flush().map_err(|error| error.to_string())?;
    if let Some(line) = lines.next() {
        line.map_err(|error| error.to_string())?;
    }
    peer.close()?;
    println!("{}", serde_json::json!({"stopped":true}));
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
