use std::time::Duration;

use futures_lite::AsyncReadExt;

use super::{CachePolicy, ReadError};

#[derive(Clone)]
pub(super) struct Transport(ureq::Agent);

impl Transport {
    pub fn new() -> Result<Self, String> {
        Ok(Self(
            ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_connect(Some(Duration::from_secs(15)))
                .timeout_recv_response(Some(Duration::from_secs(60)))
                .timeout_recv_body(Some(Duration::from_secs(120)))
                .build()
                .into(),
        ))
    }

    pub async fn request(
        &self,
        method: &str,
        url: &str,
        headers: Vec<(String, String)>,
        body: Option<String>,
        _cache: CachePolicy,
    ) -> Result<Response, String> {
        let mut request = ureq::http::Request::builder().method(method).uri(url);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let request = request
            .body(body.unwrap_or_default())
            .map_err(|_| "Could not create the Website request")?;
        let agent = self.0.clone();
        blocking::unblock(move || agent.run(request))
            .await
            .map(Response)
            .map_err(|_| "Cannot reach the server. Check your connection.".into())
    }
}

pub struct Response(ureq::http::Response<ureq::Body>);

impl Response {
    pub fn status(&self) -> u16 {
        self.0.status().as_u16()
    }
    pub fn ok(&self) -> bool {
        self.0.status().is_success()
    }
    pub fn header(&self, name: &str) -> Option<String> {
        self.0.headers().get(name).and_then(|value| value.to_str().ok()).map(str::to_owned)
    }

    pub async fn bytes(
        self,
        limit: u64,
        mut on_chunk: impl FnMut(u64),
    ) -> Result<Vec<u8>, ReadError> {
        let mut reader = blocking::Unblock::new(self.0.into_body().into_reader());
        let mut bytes = Vec::new();
        let mut chunk = vec![0_u8; 64 * 1024];
        loop {
            let count =
                reader.read(&mut chunk).await.map_err(|_| "Could not read the Website response")?;
            if count == 0 {
                return Ok(bytes);
            }
            if bytes.len() as u64 + count as u64 > limit {
                return Err(ReadError::TooLarge);
            }
            bytes.extend_from_slice(&chunk[..count]);
            on_chunk(bytes.len() as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn native_http_delivers_status_headers_body_and_bounded_progress() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with("GET /game-stg/api/check HTTP/1.1"));
                assert!(request.to_ascii_lowercase().contains("authorization: bearer test-only"));
                socket.write_all(b"HTTP/1.1 409 Conflict\r\nContent-Length: 20\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"error\":\"conflict\"}").unwrap();
            }
        });
        futures_lite::future::block_on(async {
            let client = super::super::Client::new(&format!("http://{address}/")).unwrap();
            for limit in [20, 19] {
                let response = client
                    .request(
                        "GET",
                        "/game-stg/api/check",
                        vec![("Authorization".into(), "Bearer test-only".into())],
                        None,
                        CachePolicy::Default,
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), 409);
                assert!(!response.ok());
                assert_eq!(response.header("content-type").as_deref(), Some("application/json"));
                let mut progress = Vec::new();
                let result = response.bytes(limit, |bytes| progress.push(bytes)).await;
                if limit == 20 {
                    assert_eq!(result.unwrap(), br#"{"error":"conflict"}"#);
                    assert_eq!(progress.last(), Some(&20));
                } else {
                    assert!(matches!(result, Err(ReadError::TooLarge)));
                }
            }
        });
        server.join().unwrap();
    }
}
