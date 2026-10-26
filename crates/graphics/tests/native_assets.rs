#![cfg(not(target_arch = "wasm32"))]

#[test]
fn native_resource_bytes_retire_with_their_view_owner() {
    futures_lite::future::block_on(async {
        let resource = solomon_graphics::assets::ResourceUrl::new(
            b"verified bytes".to_vec(),
            "application/octet-stream",
        )
        .unwrap();
        let url = resource.as_str().to_owned();
        assert_eq!(solomon_graphics::assets::fetch_bytes(&url).await.unwrap(), b"verified bytes");
        drop(resource);
        assert!(solomon_graphics::assets::fetch_bytes(&url).await.unwrap_err().contains("retired"));
    });
}

use solomon_graphics::assets::{NativeAssets, decode_image};

#[test]
fn packaged_assets_use_the_configured_root_and_reject_escape() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
    let assets = NativeAssets::new(root).expect("tracked packaged asset root");
    let bytes = futures_lite::future::block_on(assets.fetch("assets/stock/favicon.png"))
        .expect("the same packaged image used by the browser");
    let image = decode_image("favicon.png", &bytes).expect("decoded stock PNG");
    assert!(image.width() > 0 && image.height() > 0);
    assert_eq!(image.rgba().len(), image.width() as usize * image.height() as usize * 4);
    for path in ["../Cargo.toml", "assets/../../Cargo.toml", "file:///etc/passwd"] {
        assert!(futures_lite::future::block_on(assets.fetch(path)).is_err(), "{path}");
    }
}

#[test]
fn absent_and_corrupt_assets_are_errors_instead_of_empty_images() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
    let assets = NativeAssets::new(root).expect("tracked packaged asset root");
    assert!(
        futures_lite::future::block_on(assets.fetch("assets/stock/does-not-exist.png")).is_err()
    );
    assert!(decode_image("broken.png", b"not a PNG").is_err());
}
