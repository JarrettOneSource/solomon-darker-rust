//! Native loading of the shipped stock world and its prepared navigation. The shared
//! asset decoders validate geometry identities; this adapter only reads local files.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use solomon_assets::boneyard::{BoneyardCatalog, ModBoneyardEntry};
use solomon_assets::navigation::CatalogNavigation;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{BoneyardChoice, BoneyardSource, LoadedBoneyard};

pub struct HostContent {
    catalog: BoneyardCatalog,
    navigation_directory: PathBuf,
    navigation_catalog: Arc<Mutex<CatalogNavigation>>,
}

impl HostContent {
    pub fn load(asset_directory: &Path) -> Result<Self, String> {
        Self::load_with_mod_entries(asset_directory, Vec::new())
    }

    pub fn load_with_mod_entries(
        asset_directory: &Path,
        entries: Vec<ModBoneyardEntry>,
    ) -> Result<Self, String> {
        Ok(Self {
            catalog: BoneyardCatalog::load_embedded()?.with_mod_entries(entries)?,
            navigation_directory: asset_directory.join("assets/navigation"),
            navigation_catalog: Arc::new(Mutex::new(CatalogNavigation::default())),
        })
    }

    /// Each admitted party retains its own catalog identity across successive runs.
    /// Stock scene objects are module globals in the Website, so derived catalogs
    /// share their existing lazy graphs. The cache separately keys Mod entries by
    /// their retained Arc identity; equal bytes never merge fresh Mod owners.
    pub fn with_mod_entries(&self, entries: Vec<ModBoneyardEntry>) -> Result<Self, String> {
        Ok(Self {
            catalog: self.catalog.clone().with_mod_entries(entries)?,
            navigation_directory: self.navigation_directory.clone(),
            navigation_catalog: Arc::clone(&self.navigation_catalog),
        })
    }

    #[must_use]
    pub fn choices(&self) -> Vec<BoneyardChoice> {
        self.catalog.choices()
    }

    pub fn prepare_choice(
        &self,
        seed: [u8; 16],
        choice_id: &str,
        run_id: &str,
    ) -> Result<(LoadedBoneyard, NavigationSet), String> {
        let loaded = self
            .catalog
            .materialize_choice(choice_id, seed, run_id)
            .ok_or_else(|| format!("Boneyard content is unavailable: {choice_id}"))?;
        let navigation = self.prepare_catalog_navigation(&loaded)?;
        Ok((loaded, navigation))
    }

    pub fn prepare_run(
        &self,
        seed: [u8; 16],
        tutorial: bool,
    ) -> Result<(LoadedBoneyard, NavigationSet), String> {
        let loaded = if tutorial {
            self.catalog.materialize_tutorial(seed)
        } else {
            self.catalog.materialize(seed)
        };
        let navigation = self.prepare_catalog_navigation(&loaded)?;
        Ok((loaded, navigation))
    }

    /// Retain the source catalog entry's identity across ordinary new runs.
    pub fn prepare_catalog_navigation(
        &self,
        loaded: &LoadedBoneyard,
    ) -> Result<NavigationSet, String> {
        let mut navigation =
            self.navigation_catalog.lock().expect("catalog navigation cache is not poisoned");
        if let Some(retained) = navigation.get(&self.catalog, loaded)? {
            return Ok(retained);
        }
        let prepared = self.load_navigation(loaded)?;
        navigation.retain(&self.catalog, loaded, prepared)
    }

    /// Reloaded Mod entries have fresh Arc identities; running worlds keep their old handles.
    pub fn replace_catalog(&mut self, catalog: BoneyardCatalog) {
        self.catalog = catalog;
    }

    /// A decoded save owns a fresh scene even when its bytes equal a catalog entry.
    pub fn load_navigation(&self, loaded: &LoadedBoneyard) -> Result<NavigationSet, String> {
        if loaded.choice_source == BoneyardSource::Mod {
            return Ok(NavigationSet::for_scene(loaded));
        }
        let path = self.navigation_directory.join(format!("{}.json", loaded.source_sha256));
        let json = fs::read_to_string(&path).map_err(|error| {
            format!("Cannot load native navigation {}: {error}", path.display())
        })?;
        solomon_assets::navigation::decode_navigation(&json, loaded)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use solomon_sim::boneyard::collision::BoneyardCollisionWorld;

    #[test]
    fn new_catalog_runs_reuse_navigation_but_decoded_worlds_have_fresh_owners() {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
        let content = HostContent::load(&assets).unwrap();
        assert_eq!(content.choices().len(), 1);
        let (loaded, first) = content.prepare_choice([0; 16], "default-random", "first").unwrap();
        let bounds = loaded.scene.bounds;
        assert_eq!(loaded.run_id, "first");
        assert!(first.prepared_for(bounds, 25.0).is_none());
        first.prepare(bounds, &BoneyardCollisionWorld::from_scene(&loaded.scene), 25.0);
        let original_mesh = first.prepared_for(bounds, 25.0).unwrap();
        let party_content = content.with_mod_entries(Vec::new()).unwrap();
        let (_, party_navigation) =
            party_content.prepare_choice([0; 16], "default-random", "party").unwrap();
        assert!(Arc::ptr_eq(&original_mesh, &party_navigation.prepared_for(bounds, 25.0).unwrap()));
        let (loaded_again, second) =
            content.prepare_choice([0; 16], "default-random", "second").unwrap();
        assert_eq!(loaded_again.run_id, "second");
        assert!(Arc::ptr_eq(&original_mesh, &second.prepared_for(bounds, 25.0).unwrap()));
        assert!(content.load_navigation(&loaded).unwrap().prepared_for(bounds, 25.0).is_none());
        assert!(content.prepare_choice([0; 16], "missing", "rejected").is_err());
    }
}
