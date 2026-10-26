//! The published desktop v1 invite, including its canonical 256-bit capability.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::net::IpAddr;

#[derive(Clone)]
pub struct Invite {
    pub host: String,
    pub port: u16,
    pub revision: String,
    // Never include the capability in Debug or operational error messages.
    pub(crate) secret: String,
}

impl Invite {
    pub fn new(host: &str, port: u16, revision: &str, secret: &str) -> Result<Self, String> {
        let hostname = !host.is_empty()
            && host.len() <= 253
            && host.as_bytes()[0].is_ascii_alphanumeric()
            && host.as_bytes()[host.len() - 1].is_ascii_alphanumeric()
            && host.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".-".contains(&byte));
        if host.parse::<IpAddr>().is_err() && !hostname {
            return Err("Enter a host IP address or hostname, without a port or URL.".into());
        }
        if port == 0 {
            return Err("The peer port must be between 1 and 65535.".into());
        }
        if revision != "development"
            && (revision.len() != 40
                || !revision
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        {
            return Err("Invalid invite version.".into());
        }
        let key = URL_SAFE_NO_PAD.decode(secret).map_err(|_| "Invalid invite key.")?;
        if key.len() != 32 || URL_SAFE_NO_PAD.encode(&key) != secret {
            return Err("Invalid invite key.".into());
        }
        Ok(Self { host: host.into(), port, revision: revision.into(), secret: secret.into() })
    }

    pub fn parse(value: &str, revision: &str) -> Result<Self, String> {
        if value.len() > 1024 {
            return Err("Invalid multiplayer invite.".into());
        }
        let url = url::Url::parse(value.trim()).map_err(|_| "Invalid multiplayer invite.")?;
        if url.scheme() != "solomondarker"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || !url.path().starts_with("/v1/")
        {
            return Err("Invalid multiplayer invite.".into());
        }
        let invite = Self::new(
            url.host_str().ok_or("Invalid multiplayer invite.")?.trim_matches(['[', ']']),
            url.port().ok_or("Invalid multiplayer invite.")?,
            &url.path()[4..],
            url.fragment().ok_or("Invalid multiplayer invite.")?,
        )?;
        if invite.revision != revision {
            return Err("Both players need the same version of Solomon Darker. Download the host’s release or update both apps.".into());
        }
        Ok(invite)
    }

    pub fn url(&self) -> String {
        let host = match self.host.parse::<IpAddr>() {
            Ok(IpAddr::V6(_)) => format!("[{}]", self.host),
            _ => self.host.clone(),
        };
        format!("solomondarker://{host}:{}/v1/{}#{}", self.port, self.revision, self.secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_invites_roundtrip_and_reject_changed_versions_or_noncanonical_capabilities() {
        let key = URL_SAFE_NO_PAD.encode([17_u8; 32]);
        for host in ["127.0.0.1", "::1", "example.test", "my-host"] {
            let invite = Invite::new(host, 27843, &"a".repeat(40), &key).unwrap();
            let parsed = Invite::parse(&format!(" {} ", invite.url()), &invite.revision).unwrap();
            assert_eq!(parsed.host, host);
            assert_eq!(parsed.secret, key);
            assert_eq!(parsed.url(), invite.url());
            assert!(Invite::parse(&invite.url(), &"b".repeat(40)).is_err());
        }
        for host in ["", "https://host", "host:80", "a/b", " a", "a_", "-host", "host-"] {
            assert!(Invite::new(host, 1, "development", &key).is_err(), "{host}");
        }
        for key in [format!("{key}="), "a".repeat(43), "!".repeat(43), "a".repeat(42)] {
            assert!(Invite::new("host", 1, "development", &key).is_err());
        }
        let invite = Invite::new("host", 1, "development", &key).unwrap().url();
        for invalid in [
            invite.replace("solomondarker:", "https:"),
            invite.replace("host:", "user@host:"),
            invite.replace("#", "?x#"),
            invite.replace(":1/", "/"),
            invite.replace("/v1/", "/v2/"),
        ] {
            assert!(Invite::parse(&invalid, "development").is_err());
        }
    }
}
