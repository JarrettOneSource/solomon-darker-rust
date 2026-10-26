//! Packaged files and authored image requests without a browser or JavaScript engine.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use base64::Engine;

thread_local! {
    static RESOURCES: RefCell<BTreeMap<String, Weak<[u8]>>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT_RESOURCE: Cell<u64> = const { Cell::new(0) };
}

/// A view/session owns the bytes behind this native URL, just as it owns a browser Blob.
pub struct ResourceUrl {
    url: String,
    _bytes: Rc<[u8]>,
}

impl ResourceUrl {
    pub fn new(bytes: Vec<u8>, _content_type: &str) -> Result<Self, String> {
        let id = NEXT_RESOURCE.get().checked_add(1).ok_or("Native resource identity overflow")?;
        NEXT_RESOURCE.set(id);
        let url = format!("memory:solomon/{id}");
        let bytes: Rc<[u8]> = bytes.into();
        RESOURCES.with_borrow_mut(|resources| {
            resources.insert(url.clone(), Rc::downgrade(&bytes));
        });
        Ok(Self { url, _bytes: bytes })
    }

    pub fn as_str(&self) -> &str {
        &self.url
    }
}

impl Drop for ResourceUrl {
    fn drop(&mut self) {
        RESOURCES.with_borrow_mut(|resources| {
            resources.remove(&self.url);
        });
    }
}

fn resource_bytes(location: &str) -> Result<Option<Rc<[u8]>>, String> {
    if !location.starts_with("memory:") {
        return Ok(None);
    }
    RESOURCES
        .with_borrow(|resources| resources.get(location).and_then(Weak::upgrade))
        .map(Some)
        .ok_or_else(|| "The native content owner has retired".into())
}

#[derive(Clone)]
pub struct NativeAssets {
    root: Arc<PathBuf>,
    http: ureq::Agent,
}

impl NativeAssets {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|error| format!("Game assets at {}: {error}", root.as_ref().display()))?;
        if !root.is_dir() {
            return Err(format!("Game asset root is not a directory: {}", root.display()));
        }
        Ok(Self { root: Arc::new(root), http: ureq::Agent::new_with_defaults() })
    }

    pub async fn fetch(&self, location: &str) -> Result<Vec<u8>, String> {
        if let Some(bytes) = resource_bytes(location)? {
            return Ok(bytes.to_vec());
        }
        let assets = self.clone();
        let location = location.to_owned();
        blocking::unblock(move || assets.read(&location)).await
    }

    fn read(&self, location: &str) -> Result<Vec<u8>, String> {
        if location.starts_with("https://") || location.starts_with("http://") {
            let mut response =
                self.http.get(location).call().map_err(|error| format!("{location}: {error}"))?;
            return response
                .body_mut()
                .with_config()
                .limit(128 * 1024 * 1024)
                .read_to_vec()
                .map_err(|error| format!("{location}: {error}"));
        }
        if let Some(data) = location.strip_prefix("data:") {
            let (metadata, encoded) = data.split_once(',').ok_or("Invalid asset data URL")?;
            if !metadata.ends_with(";base64") {
                return Err("Asset data URLs must use base64 encoding".into());
            }
            return base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|error| format!("Asset data URL: {error}"));
        }
        let relative = Path::new(location.trim_start_matches('/'));
        if location.contains([':', '\\'])
            || location.starts_with("//")
            || relative.components().any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(format!("Asset path leaves the packaged directory: {location}"));
        }
        let path = self
            .root
            .join(relative)
            .canonicalize()
            .map_err(|error| format!("{location}: {error}"))?;
        if !path.starts_with(self.root.as_path()) {
            return Err(format!("Asset path leaves the packaged directory: {location}"));
        }
        std::fs::read(path).map_err(|error| format!("{location}: {error}"))
    }
}

#[derive(Clone)]
pub struct Image {
    source: String,
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}

impl Image {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn src(&self) -> String {
        self.source.clone()
    }
    pub fn natural_width(&self) -> u32 {
        self.width
    }
    pub fn natural_height(&self) -> u32 {
        self.height
    }
}

pub fn decode_image(source: &str, bytes: &[u8]) -> Result<Image, String> {
    let image =
        image::load_from_memory(bytes).map_err(|error| format!("{source}: {error}"))?.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(Image { source: source.into(), width, height, rgba: image.into_raw().into() })
}

thread_local! {
    static ASSETS: RefCell<Option<NativeAssets>> = const { RefCell::new(None) };
}

/// The application selects its verified packaged root before starting any client loads.
pub fn configure(assets: NativeAssets) {
    ASSETS.with(|slot| *slot.borrow_mut() = Some(assets));
}

pub async fn fetch_bytes(location: &str) -> Result<Vec<u8>, String> {
    if let Some(bytes) = resource_bytes(location)? {
        return Ok(bytes.to_vec());
    }
    let assets = ASSETS
        .with(|slot| slot.borrow().clone())
        .ok_or("The native game asset root has not been configured")?;
    assets.fetch(location).await
}

pub async fn load_image(location: &str) -> Result<Image, String> {
    let bytes = fetch_bytes(location).await?;
    let source = location.to_owned();
    blocking::unblock(move || decode_image(&source, &bytes)).await
}
