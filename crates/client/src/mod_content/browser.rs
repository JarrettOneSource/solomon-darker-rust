use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[derive(Clone)]
pub(super) struct ContentCache {
    cache: Option<web_sys::Cache>,
    subtle: web_sys::SubtleCrypto,
}

impl ContentCache {
    pub async fn open() -> Result<Self, String> {
        let window = web_sys::window().ok_or("The game has no browser window")?;
        let property = Reflect::get(&window, &"caches".into()).map_err(describe)?;
        let cache = if property.is_undefined() || property.is_null() {
            // Source permits browsers without CacheStorage; verification remains mandatory.
            None
        } else {
            Some(
                JsFuture::from(
                    window.caches().map_err(describe)?.open("solomon-dark-game-content-v1"),
                )
                .await
                .map_err(describe)?
                .dyn_into()
                .map_err(describe)?,
            )
        };
        Ok(Self { cache, subtle: window.crypto().map_err(describe)?.subtle() })
    }

    pub async fn read(
        &self,
        url: &str,
        _digest: &str,
        _expected: u64,
    ) -> Result<Option<Vec<u8>>, String> {
        let Some(cache) = &self.cache else { return Ok(None) };
        let value = JsFuture::from(cache.match_with_str(url)).await.map_err(describe)?;
        if value.is_undefined() {
            return Ok(None);
        }
        let response: web_sys::Response = value.dyn_into().map_err(describe)?;
        let buffer =
            JsFuture::from(response.array_buffer().map_err(describe)?).await.map_err(describe)?;
        Ok(Some(Uint8Array::new(&buffer).to_vec()))
    }

    pub async fn remove(&self, url: &str, _digest: &str) -> Result<(), String> {
        if let Some(cache) = &self.cache {
            JsFuture::from(cache.delete_with_str(url)).await.map_err(describe)?;
        }
        Ok(())
    }

    pub async fn write(
        &self,
        url: &str,
        _digest: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> Result<(), String> {
        if let Some(cache) = &self.cache {
            let response =
                web_sys::Response::new_with_opt_js_u8_array(Some(&Uint8Array::from(bytes)))
                    .map_err(describe)?;
            response
                .headers()
                .set("cache-control", "public, max-age=31536000, immutable")
                .map_err(describe)?;
            response.headers().set("content-type", content_type).map_err(describe)?;
            JsFuture::from(cache.put_with_str(url, &response)).await.map_err(describe)?;
        }
        Ok(())
    }

    pub async fn verify(&self, expected: &str, bytes: Vec<u8>) -> Result<Option<Vec<u8>>, String> {
        let digest = JsFuture::from(
            self.subtle.digest_with_str_and_u8_array("SHA-256", &bytes).map_err(describe)?,
        )
        .await
        .map_err(describe)?;
        let hex: String =
            Uint8Array::new(&digest).to_vec().iter().map(|byte| format!("{byte:02x}")).collect();
        Ok((hex == expected).then_some(bytes))
    }
}

fn describe(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| Reflect::get(&error, &"message".into()).ok()?.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}
