//! Browser fetch for the shared, identity-checked native navigation decoder.

use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};

pub async fn load_navigation(loaded: &LoadedBoneyard) -> Result<NavigationSet, String> {
    if loaded.choice_source == BoneyardSource::Mod {
        return Ok(NavigationSet::for_scene(loaded));
    }
    let path = format!("assets/navigation/{}.json", loaded.source_sha256);
    let bytes = solomon_graphics::assets::fetch_bytes(&path).await?;
    let json =
        std::str::from_utf8(&bytes).map_err(|error| format!("Boneyard navigation: {error}"))?;
    solomon_assets::navigation::decode_navigation(json, loaded)
}
