//! Verified package bytes from Website `game-content-cache.ts`.
//! The browser streams through the mounted account API, then admits only the descriptor's
//! exact length and SHA-256 to CacheStorage and the image/audio/document consumers.

use std::collections::BTreeMap;
use std::fmt::Write;

use js_sys::{Array, Reflect, Uint8Array};
use solomon_sim::modding::assets::GameModAsset;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, Cache, ReadableStreamDefaultReader, Response, SubtleCrypto};

use crate::hosted::WebsiteApi;

const CACHE_NAME: &str = "solomon-dark-game-content-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameModContentLoadError {
    pub mod_id: String,
    pub message: String,
}

impl GameModContentLoadError {
    fn asset(asset: &GameModAsset, message: impl Into<String>) -> Self {
        Self { mod_id: asset.mod_id.clone(), message: message.into() }
    }

    fn platform(message: &str, error: JsValue) -> Self {
        Self { mod_id: String::new(), message: format!("{message}: {}", describe(&error)) }
    }
}

impl std::fmt::Display for GameModContentLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GameModContentLoadError {}

/// Progress reports bytes actually read from the response, including cache hits.
pub struct GameContentDownloadProgress<'a> {
    pub active: Option<&'a GameModAsset>,
    pub completed_bytes: u64,
    pub total_bytes: u64,
}

/// The accepted descriptors remain owned by the session. This is only their browser I/O.
#[derive(Clone)]
pub struct ModContentCache {
    api: WebsiteApi,
    cache: Option<Cache>,
    subtle: SubtleCrypto,
}

impl ModContentCache {
    pub async fn open(api: WebsiteApi) -> Result<Self, GameModContentLoadError> {
        let window = web_sys::window().ok_or_else(|| GameModContentLoadError {
            mod_id: String::new(),
            message: "The game has no browser window".into(),
        })?;
        let cache_property =
            Reflect::get(&window, &JsValue::from_str("caches")).map_err(|error| {
                GameModContentLoadError::platform("Could not open content cache", error)
            })?;
        let cache = if cache_property.is_undefined() || cache_property.is_null() {
            // Source permits browsers without CacheStorage; verification remains mandatory.
            None
        } else {
            let storage = window.caches().map_err(|error| {
                GameModContentLoadError::platform("Could not open content cache", error)
            })?;
            let value = JsFuture::from(storage.open(CACHE_NAME)).await.map_err(|error| {
                GameModContentLoadError::platform("Could not open content cache", error)
            })?;
            Some(value.dyn_into::<Cache>().map_err(|error| {
                GameModContentLoadError::platform(
                    "The browser returned an invalid content cache",
                    error,
                )
            })?)
        };
        let subtle = window
            .crypto()
            .map_err(|error| {
                GameModContentLoadError::platform("Content verification is unavailable", error)
            })?
            .subtle();
        Ok(Self { api, cache, subtle })
    }

    pub async fn prefetch(
        &self,
        assets: &[GameModAsset],
        mut on_progress: impl FnMut(GameContentDownloadProgress<'_>),
    ) -> Result<(), GameModContentLoadError> {
        let unique = unique_assets(assets)?;
        let total_bytes = unique.iter().map(|asset| asset.byte_length).sum();
        let mut completed_bytes = 0;
        on_progress(GameContentDownloadProgress { active: None, completed_bytes, total_bytes });
        for asset in unique {
            let bytes = self
                .read(asset, |received| {
                    on_progress(GameContentDownloadProgress {
                        active: Some(asset),
                        completed_bytes: completed_bytes + received,
                        total_bytes,
                    });
                })
                .await?;
            completed_bytes += bytes.len() as u64;
            on_progress(GameContentDownloadProgress {
                active: Some(asset),
                completed_bytes,
                total_bytes,
            });
        }
        on_progress(GameContentDownloadProgress { active: None, completed_bytes, total_bytes });
        Ok(())
    }

    /// Revalidates cached bytes before a consumer creates a texture, script or audio buffer.
    pub async fn bytes(&self, asset: &GameModAsset) -> Result<Vec<u8>, GameModContentLoadError> {
        self.read(asset, |_| {}).await
    }

    /// Source `gameContentUrl`, resolved within this Website mount. HTML media
    /// uses this same-origin address after its content bytes have been verified.
    pub(crate) fn content_url(&self, asset: &GameModAsset) -> String {
        self.api.mount().path(&format!("/api/game/content/{}", asset.sha256))
    }

    pub async fn object_url(
        &self,
        asset: &GameModAsset,
    ) -> Result<ModContentUrl, GameModContentLoadError> {
        let bytes = self.bytes(asset).await?;
        let parts = Array::new();
        parts.push(&Uint8Array::from(bytes.as_slice()));
        let options = BlobPropertyBag::new();
        options.set_type(&asset.content_type);
        let blob =
            Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(|_| {
                GameModContentLoadError::asset(asset, "Could not prepare verified content")
            })?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|_| {
            GameModContentLoadError::asset(asset, "Could not open verified content")
        })?;
        Ok(ModContentUrl { url })
    }

    async fn read(
        &self,
        asset: &GameModAsset,
        mut on_chunk: impl FnMut(u64),
    ) -> Result<Vec<u8>, GameModContentLoadError> {
        let url = self.content_url(asset);
        if let Some(cache) = &self.cache {
            let cached = JsFuture::from(cache.match_with_str(&url)).await.map_err(|_| {
                GameModContentLoadError::asset(
                    asset,
                    format!("Could not read cached content for {}:{}.", asset.mod_id, asset.path),
                )
            })?;
            if !cached.is_undefined() {
                let response = cached.dyn_into::<Response>().map_err(|_| {
                    GameModContentLoadError::asset(
                        asset,
                        "The browser returned invalid cached content",
                    )
                })?;
                let buffer = response.array_buffer().map_err(|_| cached_read_error(asset))?;
                let buffer = JsFuture::from(buffer).await.map_err(|_| cached_read_error(asset))?;
                let bytes = Uint8Array::new(&buffer).to_vec();
                if self.verified(asset, &bytes).await? {
                    return Ok(bytes);
                }
                JsFuture::from(cache.delete_with_str(&url)).await.map_err(|error| {
                    GameModContentLoadError::platform(
                        "Could not discard invalid cached content",
                        error,
                    )
                })?;
            }
        }
        let response =
            self.api.content_download(&asset.sha256).await.map_err(|_| download_error(asset))?;
        let body = response.body().ok_or_else(|| download_error(asset))?;
        let reader = body
            .get_reader()
            .dyn_into::<ReadableStreamDefaultReader>()
            .map_err(|_| download_error(asset))?;
        let mut bytes = Vec::new();
        loop {
            let next = JsFuture::from(reader.read()).await.map_err(|_| download_error(asset))?;
            let done = Reflect::get(&next, &JsValue::from_str("done"))
                .map_err(|_| download_error(asset))?;
            if done.as_bool() == Some(true) {
                break;
            }
            let value = Reflect::get(&next, &JsValue::from_str("value"))
                .map_err(|_| download_error(asset))?;
            let chunk = value.dyn_into::<Uint8Array>().map_err(|_| download_error(asset))?;
            if bytes.len() as u64 + u64::from(chunk.length()) > asset.byte_length {
                return Err(GameModContentLoadError::asset(
                    asset,
                    format!("Downloaded content is too large: {}:{}.", asset.mod_id, asset.path,),
                ));
            }
            bytes.extend(chunk.to_vec());
            on_chunk(bytes.len() as u64);
        }
        reader.release_lock();
        if !self.verified(asset, &bytes).await? {
            return Err(GameModContentLoadError::asset(
                asset,
                format!("Downloaded content failed verification: {}:{}.", asset.mod_id, asset.path,),
            ));
        }
        if let Some(cache) = &self.cache {
            let response =
                Response::new_with_opt_js_u8_array(Some(&Uint8Array::from(bytes.as_slice())))
                    .map_err(|error| {
                        GameModContentLoadError::platform("Could not prepare cached content", error)
                    })?;
            for (name, value) in [
                ("cache-control", "public, max-age=31536000, immutable"),
                ("content-type", asset.content_type.as_str()),
            ] {
                response.headers().set(name, value).map_err(|error| {
                    GameModContentLoadError::platform("Could not prepare cached content", error)
                })?;
            }
            JsFuture::from(cache.put_with_str(&url, &response)).await.map_err(|error| {
                GameModContentLoadError::platform("Could not save verified content", error)
            })?;
        }
        Ok(bytes)
    }

    async fn verified(
        &self,
        asset: &GameModAsset,
        bytes: &[u8],
    ) -> Result<bool, GameModContentLoadError> {
        if bytes.len() as u64 != asset.byte_length {
            return Ok(false);
        }
        let promise =
            self.subtle.digest_with_str_and_u8_array("SHA-256", bytes).map_err(|error| {
                GameModContentLoadError::platform("Could not verify content", error)
            })?;
        let digest = JsFuture::from(promise).await.map_err(|error| {
            GameModContentLoadError::platform("Could not verify content", error)
        })?;
        let mut hex = String::with_capacity(64);
        for byte in Uint8Array::new(&digest).to_vec() {
            write!(&mut hex, "{byte:02x}").expect("writing a digest to a String cannot fail");
        }
        Ok(hex == asset.sha256)
    }
}

/// A consumer retains this lease as long as its decoded image/audio source can use the URL.
pub struct ModContentUrl {
    url: String,
}

impl ModContentUrl {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }
}

impl Drop for ModContentUrl {
    fn drop(&mut self) {
        // Revocation is idempotent; a browser shutdown cannot leave a live resource behind.
        let _ = web_sys::Url::revoke_object_url(&self.url);
    }
}

fn unique_assets(assets: &[GameModAsset]) -> Result<Vec<&GameModAsset>, GameModContentLoadError> {
    let mut seen: BTreeMap<&str, &GameModAsset> = BTreeMap::new();
    let mut unique = Vec::new();
    for asset in assets {
        if let Some(existing) = seen.get(asset.sha256.as_str()) {
            if existing.byte_length != asset.byte_length {
                return Err(GameModContentLoadError::asset(
                    asset,
                    format!("Content identity has conflicting sizes: {}.", asset.sha256,),
                ));
            }
            continue;
        }
        seen.insert(&asset.sha256, asset);
        unique.push(asset);
    }
    Ok(unique)
}

fn cached_read_error(asset: &GameModAsset) -> GameModContentLoadError {
    GameModContentLoadError::asset(
        asset,
        format!("Could not read cached content for {}:{}.", asset.mod_id, asset.path,),
    )
}

fn download_error(asset: &GameModAsset) -> GameModContentLoadError {
    GameModContentLoadError::asset(
        asset,
        format!("Could not download {}:{}.", asset.mod_id, asset.path,),
    )
}

fn describe(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| Reflect::get(error, &JsValue::from_str("message")).ok()?.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immutable_content_deduplicates_by_hash_and_rejects_conflicting_sizes() {
        let first = GameModAsset {
            byte_length: 19,
            content_type: "image/png".into(),
            kind: "image".into(),
            mod_id: "example.first".into(),
            path: "item.png".into(),
            sha256: "a".repeat(64),
        };
        let mut second = first.clone();
        second.mod_id = "example.second".into();
        let assets = [first.clone(), second.clone()];
        assert_eq!(unique_assets(&assets).unwrap(), vec![&first]);
        second.byte_length += 1;
        let failure = unique_assets(&[first, second]).unwrap_err();
        assert_eq!(failure.mod_id, "example.second");
        assert_eq!(
            failure.message,
            format!("Content identity has conflicting sizes: {}.", "a".repeat(64))
        );
    }
}
