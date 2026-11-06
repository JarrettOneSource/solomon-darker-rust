use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Window};

use super::{CachePolicy, ReadError};

#[derive(Clone)]
pub(super) struct Transport(Window);

struct RequestAbort(web_sys::AbortController);
impl Drop for RequestAbort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Transport {
    pub fn new() -> Result<Self, String> {
        web_sys::window().map(Self).ok_or_else(|| "The game has no browser window".into())
    }

    pub async fn request(
        &self,
        method: &str,
        url: &str,
        headers: Vec<(String, String)>,
        body: Option<String>,
        cache: CachePolicy,
    ) -> Result<Response, String> {
        let abort = RequestAbort(
            web_sys::AbortController::new()
                .map_err(|_| "Could not create the Website request cancellation")?,
        );
        let options = RequestInit::new();
        options.set_signal(Some(&abort.0.signal()));
        options.set_method(method);
        options.set_credentials(web_sys::RequestCredentials::SameOrigin);
        options.set_cache(match cache {
            CachePolicy::Default => web_sys::RequestCache::Default,
            CachePolicy::Immutable => web_sys::RequestCache::ForceCache,
        });
        if let Some(body) = body {
            options.set_body(&JsValue::from_str(&body));
        }
        let request = Request::new_with_str_and_init(url, &options)
            .map_err(|_| "Could not create the Website request")?;
        for (name, value) in headers {
            request.headers().set(&name, &value).map_err(|_| "Could not set request headers")?;
        }
        JsFuture::from(self.0.fetch_with_request(&request))
            .await
            .map_err(|_| "Cannot reach the server. Check your connection.")?
            .dyn_into::<web_sys::Response>()
            .map(|response| Response { response, _abort: abort })
            .map_err(|_| "The Website returned an invalid response".into())
    }
}

pub struct Response {
    pub(super) response: web_sys::Response,
    _abort: RequestAbort,
}

impl Response {
    pub fn status(&self) -> u16 {
        self.response.status()
    }
    pub fn ok(&self) -> bool {
        self.response.ok()
    }
    pub fn header(&self, name: &str) -> Option<String> {
        self.response.headers().get(name).ok().flatten()
    }

    pub async fn bytes(
        self,
        limit: u64,
        mut on_chunk: impl FnMut(u64),
    ) -> Result<Vec<u8>, ReadError> {
        let Some(body) = self.response.body() else { return Ok(Vec::new()) };
        let reader = body
            .get_reader()
            .dyn_into::<web_sys::ReadableStreamDefaultReader>()
            .map_err(|_| "Could not read the Website response")?;
        let result = async {
            let mut bytes = Vec::new();
            loop {
                let next = JsFuture::from(reader.read())
                    .await
                    .map_err(|_| "Could not read the Website response")?;
                if Reflect::get(&next, &"done".into()).ok().and_then(|value| value.as_bool())
                    == Some(true)
                {
                    return Ok(bytes);
                }
                let chunk = Reflect::get(&next, &"value".into())
                    .ok()
                    .and_then(|value| value.dyn_into::<Uint8Array>().ok())
                    .ok_or("The Website returned an invalid response stream")?;
                if bytes.len() as u64 + u64::from(chunk.length()) > limit {
                    return Err(ReadError::TooLarge);
                }
                bytes.extend(chunk.to_vec());
                on_chunk(bytes.len() as u64);
            }
        }
        .await;
        if result.is_err() && JsFuture::from(reader.cancel()).await.is_err() {
            crate::platform::warn("Could not cancel the failed Website response stream");
        }
        reader.release_lock();
        result
    }
}
