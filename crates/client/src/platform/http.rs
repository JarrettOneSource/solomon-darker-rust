//! HTTP ownership for Website controls and verified content downloads.

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
pub use browser::Response;
#[cfg(target_arch = "wasm32")]
use browser::Transport;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Response;
#[cfg(not(target_arch = "wasm32"))]
use native::Transport;

#[derive(Clone, Copy)]
pub enum CachePolicy {
    Default,
    Immutable,
}

#[derive(Debug)]
pub enum ReadError {
    TooLarge,
    Transport(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("The Website response exceeds the allowed size"),
            Self::Transport(message) => formatter.write_str(message),
        }
    }
}
impl std::error::Error for ReadError {}
impl From<&str> for ReadError {
    fn from(message: &str) -> Self {
        Self::Transport(message.into())
    }
}

#[derive(Clone)]
pub struct Client {
    base: url::Url,
    transport: Transport,
}

impl Client {
    pub fn new(base: &str) -> Result<Self, String> {
        let base = url::Url::parse(base).map_err(|_| "The Website address is invalid")?;
        if !matches!(base.scheme(), "https" | "http")
            || !base.username().is_empty()
            || base.password().is_some()
        {
            return Err("The Website address is invalid".into());
        }
        Ok(Self { base, transport: Transport::new()? })
    }

    pub fn base(&self) -> &url::Url {
        &self.base
    }

    /// Account headers must only accompany requests to this configured Website.
    pub fn resolve(&self, path: &str) -> Result<url::Url, String> {
        let url = self.base.join(path).map_err(|_| "The Website request address is invalid")?;
        if url.origin() != self.base.origin()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("The request does not belong to this Website".into());
        }
        Ok(url)
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        headers: Vec<(String, String)>,
        body: Option<String>,
        cache: CachePolicy,
    ) -> Result<Response, String> {
        self.transport.request(method, self.resolve(path)?.as_str(), headers, body, cache).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_requests_retain_mounts_and_reject_other_origins_or_credentials() {
        let client = Client::new("https://example.test/game-stg/game/").unwrap();
        assert_eq!(
            client.resolve("/game-stg/api/auth/me").unwrap().as_str(),
            "https://example.test/game-stg/api/auth/me"
        );
        assert!(client.resolve("https://elsewhere.test/api/auth/me").is_err());
        assert!(client.resolve("https://user@example.test/api/auth/me").is_err());
        assert!(client.resolve("data:text/plain,hello").is_err());
        assert!(Client::new("file:///etc/passwd").is_err());
    }
}
