//! Navigation identity for retained source catalog entries.
//!
//! Website `host/boneyard-catalog.ts` returns the same template/Mod entry scene object
//! across materializations. `boneyard-collision.ts` uses that object as the WeakMap key
//! in `boneyard-enemy-navigation.ts`, so a later run retains its first lazy mesh.
//! Decoded saves have a fresh scene identity and must use the ordinary fresh loader.

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};

use crate::boneyard::{BoneyardCatalog, ModBoneyardEntry};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct StockScene {
    source_sha256: String,
    geometry_sha256: String,
}

#[derive(Debug)]
struct ModNavigation {
    owner: Weak<ModBoneyardEntry>,
    navigation: NavigationSet,
}

enum SceneOwner {
    Stock(StockScene),
    Mod(Arc<ModBoneyardEntry>),
}

/// One host/browser content catalog's retained navigation owners. No global cache is used.
#[derive(Debug, Default)]
pub struct CatalogNavigation {
    stock: BTreeMap<StockScene, NavigationSet>,
    mods: Vec<ModNavigation>,
}

impl CatalogNavigation {
    /// Borrow the retained entry's graph owner before preparing its next run.
    pub fn get(
        &self,
        catalog: &BoneyardCatalog,
        loaded: &LoadedBoneyard,
    ) -> Result<Option<NavigationSet>, String> {
        Ok(self.navigation_for(&scene_owner(catalog, loaded)?))
    }

    /// Keep the first owner for this catalog scene, including any lazy gate meshes.
    /// A newly prepared duplicate never replaces an already active graph owner.
    pub fn retain(
        &mut self,
        catalog: &BoneyardCatalog,
        loaded: &LoadedBoneyard,
        navigation: NavigationSet,
    ) -> Result<NavigationSet, String> {
        if navigation.source_sha256 != loaded.source_sha256
            || navigation.geometry_sha256 != loaded.geometry_sha256
        {
            return Err("Boneyard navigation identity does not match the catalog run".into());
        }
        let owner = scene_owner(catalog, loaded)?;
        if let Some(retained) = self.navigation_for(&owner) {
            return Ok(retained);
        }
        // Source WeakMap entries disappear with their scene objects. Active worlds
        // retain their own NavigationSet handles after a catalog entry is replaced.
        self.mods.retain(|entry| entry.owner.strong_count() > 0);
        match owner {
            SceneOwner::Stock(key) => {
                self.stock.insert(key, navigation.clone());
            }
            SceneOwner::Mod(owner) => {
                self.mods.push(ModNavigation {
                    owner: Arc::downgrade(&owner),
                    navigation: navigation.clone(),
                });
            }
        }
        Ok(navigation)
    }

    fn navigation_for(&self, owner: &SceneOwner) -> Option<NavigationSet> {
        match owner {
            SceneOwner::Stock(key) => self.stock.get(key).cloned(),
            SceneOwner::Mod(owner) => {
                for entry in &self.mods {
                    if let Some(retained_owner) = entry.owner.upgrade()
                        && Arc::ptr_eq(&retained_owner, owner)
                    {
                        return Some(entry.navigation.clone());
                    }
                }
                None
            }
        }
    }
}

fn scene_owner(catalog: &BoneyardCatalog, loaded: &LoadedBoneyard) -> Result<SceneOwner, String> {
    if loaded.choice_source == BoneyardSource::Mod {
        let Some(entry) = catalog.mod_entry(&loaded.choice_id) else {
            return Err("Boneyard navigation has no retained Mod catalog entry".into());
        };
        if entry.source_sha256 != loaded.source_sha256
            || entry.geometry_sha256 != loaded.geometry_sha256
            || entry.scene != loaded.scene
        {
            return Err("Boneyard navigation does not match its retained Mod scene".into());
        }
        return Ok(SceneOwner::Mod(Arc::clone(entry)));
    }
    let template = if loaded.choice_id == "stock-tutorial" {
        Some(&catalog.tutorial)
    } else if loaded.choice_id == "default-random" {
        catalog.templates.iter().find(|template| template.source_sha256 == loaded.source_sha256)
    } else {
        None
    };
    let Some(template) = template else {
        return Err("Boneyard navigation has no retained stock catalog entry".into());
    };
    if template.source_sha256 != loaded.source_sha256
        || template.geometry_sha256 != loaded.geometry_sha256
        || template.scene != loaded.scene
    {
        return Err("Boneyard navigation does not match its retained stock scene".into());
    }
    Ok(SceneOwner::Stock(StockScene {
        source_sha256: loaded.source_sha256.clone(),
        geometry_sha256: loaded.geometry_sha256.clone(),
    }))
}
