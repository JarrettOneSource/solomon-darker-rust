//! Fetching stock bundles and atlas images from the served `assets/` directory.

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlImageElement, Response};

/// Verified bytes stay reachable until their owning view or media source retires.
pub struct ResourceUrl(String);

impl ResourceUrl {
    pub fn new(bytes: Vec<u8>, content_type: &str) -> Result<Self, String> {
        let parts = js_sys::Array::new();
        parts.push(&js_sys::Uint8Array::from(bytes.as_slice()));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(content_type);
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
            .map_err(|_| "Could not prepare verified content")?;
        web_sys::Url::create_object_url_with_blob(&blob)
            .map(Self)
            .map_err(|_| "Could not open verified content".into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Drop for ResourceUrl {
    fn drop(&mut self) {
        // Revocation is idempotent and cannot invalidate already decoded pixels.
        let _ = web_sys::Url::revoke_object_url(&self.0);
    }
}

pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let response: Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|error| describe(url, &error))?
        .dyn_into()
        .map_err(|_| format!("{url}: fetch did not return a Response"))?;
    if !response.ok() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(|error| describe(url, &error))?)
        .await
        .map_err(|error| describe(url, &error))?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// Loads and fully decodes an image so the texture upload never sees a partial bitmap.
pub async fn load_image(url: &str) -> Result<HtmlImageElement, String> {
    let image = HtmlImageElement::new().map_err(|error| describe(url, &error))?;
    let mut on_load: Option<Closure<dyn FnMut()>> = None;
    let mut on_error: Option<Closure<dyn FnMut()>> = None;
    let loaded = js_sys::Promise::new(&mut |resolve, reject| {
        let resolve = resolve.clone();
        let reject = reject.clone();
        let success = Closure::<dyn FnMut()>::new(move || {
            // Promise settlement functions supplied by the browser do not throw.
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        let failure = Closure::<dyn FnMut()>::new(move || {
            let _ =
                reject.call1(&JsValue::UNDEFINED, &JsValue::from_str("could not load game asset"));
        });
        image.set_onload(Some(success.as_ref().unchecked_ref()));
        image.set_onerror(Some(failure.as_ref().unchecked_ref()));
        on_load = Some(success);
        on_error = Some(failure);
    });
    image.set_src(url);
    let result = JsFuture::from(loaded).await;
    image.set_onload(None);
    image.set_onerror(None);
    drop((on_load, on_error));
    result.map_err(|error| describe(url, &error))?;
    // `game-assets.ts::loadGameImage`: onload is authoritative. Chromium may reject
    // decode after discarding an already decoded image in a memory-constrained session.
    let _ = JsFuture::from(image.decode()).await;
    Ok(image)
}

fn describe(url: &str, error: &JsValue) -> String {
    let detail = error
        .as_string()
        .or_else(|| js_sys::Reflect::get(error, &JsValue::from_str("message")).ok()?.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    format!("{url}: {detail}")
}
