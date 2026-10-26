//! Verified package bytes from the mounted account API. Both platform caches admit
//! only the descriptor's exact length and SHA-256 before image, audio or document use.

use crate::hosted::WebsiteApi;
use crate::platform::http::ReadError;
use solomon_sim::modding::assets::GameModAsset;
use std::collections::BTreeMap;
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
use browser::ContentCache;
#[cfg(not(target_arch = "wasm32"))]
use native::ContentCache;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameModContentLoadError {
    pub mod_id: String,
    pub message: String,
}

impl GameModContentLoadError {
    fn asset(asset: &GameModAsset, message: impl Into<String>) -> Self {
        Self { mod_id: asset.mod_id.clone(), message: message.into() }
    }

    fn platform(message: &str, error: String) -> Self {
        Self { mod_id: String::new(), message: format!("{message}: {error}") }
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

/// The accepted descriptors remain owned by the session; this owner verifies and caches their bytes.
#[derive(Clone)]
pub struct ModContentCache {
    api: WebsiteApi,
    cache: ContentCache,
}

impl ModContentCache {
    pub async fn open(api: WebsiteApi) -> Result<Self, GameModContentLoadError> {
        let cache = ContentCache::open().await.map_err(|error| {
            GameModContentLoadError::platform("Could not open content cache", error)
        })?;
        Ok(Self { api, cache })
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

    /// Resolve content within the configured Website mount. Browser media uses
    /// this same-origin address after its bytes have been verified.
    pub(crate) fn content_url(&self, asset: &GameModAsset) -> String {
        self.api.mount().path(&format!("/api/game/content/{}", asset.sha256))
    }

    pub async fn object_url(
        &self,
        asset: &GameModAsset,
    ) -> Result<ModContentUrl, GameModContentLoadError> {
        let bytes = self.bytes(asset).await?;
        let url = solomon_graphics::assets::ResourceUrl::new(bytes, &asset.content_type)
            .map_err(|error| GameModContentLoadError::asset(asset, error))?;
        Ok(ModContentUrl { url })
    }

    async fn read(
        &self,
        asset: &GameModAsset,
        on_chunk: impl FnMut(u64),
    ) -> Result<Vec<u8>, GameModContentLoadError> {
        let url = self.content_url(asset);
        if let Some(bytes) = self
            .cache
            .read(&url, &asset.sha256, asset.byte_length)
            .await
            .map_err(|_| cached_read_error(asset))?
        {
            if let Some(bytes) = self.verified(asset, bytes).await? {
                return Ok(bytes);
            }
            self.cache.remove(&url, &asset.sha256).await.map_err(|error| {
                GameModContentLoadError::platform("Could not discard invalid cached content", error)
            })?;
        }
        let response =
            self.api.content_download(&asset.sha256).await.map_err(|_| download_error(asset))?;
        let bytes =
            response.bytes(asset.byte_length, on_chunk).await.map_err(|error| match error {
                ReadError::TooLarge => GameModContentLoadError::asset(
                    asset,
                    format!("Downloaded content is too large: {}:{}.", asset.mod_id, asset.path),
                ),
                ReadError::Transport(_) => download_error(asset),
            })?;
        let bytes = self.verified(asset, bytes).await?.ok_or_else(|| {
            GameModContentLoadError::asset(
                asset,
                format!("Downloaded content failed verification: {}:{}.", asset.mod_id, asset.path),
            )
        })?;
        self.cache.write(&url, &asset.sha256, &bytes, &asset.content_type).await.map_err(
            |error| GameModContentLoadError::platform("Could not save verified content", error),
        )?;
        Ok(bytes)
    }

    async fn verified(
        &self,
        asset: &GameModAsset,
        bytes: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, GameModContentLoadError> {
        if bytes.len() as u64 != asset.byte_length {
            return Ok(None);
        }
        self.cache
            .verify(&asset.sha256, bytes)
            .await
            .map_err(|error| GameModContentLoadError::platform("Could not verify content", error))
    }
}

/// Retained until its decoded image/audio consumer releases the source.
pub struct ModContentUrl {
    url: solomon_graphics::assets::ResourceUrl,
}

impl ModContentUrl {
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.url.as_str()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_content_reopens_offline_and_replaces_corrupt_bytes_only_after_verification() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let directory = std::env::temp_dir()
            .join(format!("solomon-content-{}", crate::platform::random_uuid().unwrap()));
        crate::storage::configure_native_storage(directory.clone()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        crate::storage::write_preference(&format!("sdr.token@http://{address}"), Some("test-only"))
            .unwrap();
        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
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
                assert!(request.starts_with(&format!("GET /api/game/content/{digest} HTTP/1.1")));
                assert!(request.to_ascii_lowercase().contains("authorization: bearer test-only"));
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc",
                    )
                    .unwrap();
            }
        });
        futures_lite::future::block_on(async {
            let api = WebsiteApi::new(
                &format!("http://{address}/"),
                crate::hosted::WebsiteMount::default(),
            )
            .unwrap();
            assert!(
                WebsiteApi::new("https://elsewhere.test/", crate::hosted::WebsiteMount::default())
                    .unwrap()
                    .token()
                    .unwrap()
                    .is_none(),
                "an account token cannot cross Website origins"
            );
            let cache = ModContentCache::open(api.clone()).await.unwrap();
            let asset = GameModAsset {
                byte_length: 3,
                content_type: "text/plain".into(),
                kind: "text".into(),
                mod_id: "test.content".into(),
                path: "test.txt".into(),
                sha256: digest.into(),
            };
            let mut progress = Vec::new();
            assert_eq!(cache.read(&asset, |read| progress.push(read)).await.unwrap(), b"abc");
            assert_eq!(progress.last(), Some(&3));
            let file = directory.join("content-v1").join(digest);
            std::fs::write(&file, b"abd").unwrap();
            assert_eq!(cache.bytes(&asset).await.unwrap(), b"abc");
            assert_eq!(std::fs::read(&file).unwrap(), b"abc");
            server.join().unwrap();
            let reopened = ModContentCache::open(api).await.unwrap();
            assert_eq!(
                reopened.bytes(&asset).await.unwrap(),
                b"abc",
                "verified cache works with the server stopped"
            );
            let resource = reopened.object_url(&asset).await.unwrap();
            let url = resource.as_str().to_owned();
            assert_eq!(solomon_graphics::assets::fetch_bytes(&url).await.unwrap(), b"abc");
            drop(resource);
            assert!(solomon_graphics::assets::fetch_bytes(&url).await.is_err());
            let mut invalid = asset;
            invalid.sha256 = "../escape".into();
            assert!(reopened.bytes(&invalid).await.is_err());
        });
        std::fs::remove_dir_all(directory).unwrap();
    }

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
