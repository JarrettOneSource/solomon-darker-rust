//! Loads retained stock Boneyards and decodes authored binary scene geometry.
//!
//! Reference: `Website/frontend/src/game/host/boneyard-catalog.ts`. The Website selects
//! a template by the seed's first big-endian word. The JSON is copied without changing its
//! geometry by `tools/website-oracle/boneyard-goldens.mjs`; no JavaScript runs at game time.

mod binary;
mod source_value;

pub use binary::{decode_boneyard, parse_editor_boneyard};
pub use source_value::boneyard_scene_source_value;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardChoice, BoneyardFence, BoneyardObject, BoneyardRoad, BoneyardScene,
    BoneyardSource, BoneyardSpawn, BoneyardSprite, BoneyardTemplate, BoneyardTerrain,
    LoadedBoneyard, SolomonDig,
};

const EMBEDDED_BANK: &str = include_str!("../../../web/assets/boneyards.json");

/// One accepted authored scene from `host/boneyard-catalog.ts`. Catalog entry
/// identity also owns the source navigation cache lifetime, independently of hashes.
#[derive(Clone, Debug, PartialEq)]
pub struct ModBoneyardEntry {
    pub choice: BoneyardChoice,
    pub geometry_sha256: String,
    pub scene: BoneyardScene,
    pub source_sha256: String,
    pub web_lua_content_id: Option<String>,
}

/// Retained native generator outputs and the separate stock tutorial.
#[derive(Clone, Debug)]
pub struct BoneyardCatalog {
    pub templates: Vec<BoneyardTemplate>,
    pub tutorial: BoneyardTemplate,
    mod_entries: BTreeMap<String, Arc<ModBoneyardEntry>>,
    mod_entry_order: Vec<String>,
}

impl BoneyardCatalog {
    pub fn load_embedded() -> Result<Self, String> {
        let document: CatalogDocument = serde_json::from_str(EMBEDDED_BANK)
            .map_err(|error| format!("Boneyard bank: {error}"))?;
        if document.templates.is_empty() {
            return Err("Boneyard bank contains no stock templates".into());
        }
        let templates: Vec<BoneyardTemplate> =
            document.templates.into_iter().map(Into::into).collect();
        let tutorial = document.tutorial.into();
        for template in templates.iter().chain(std::iter::once(&tutorial)) {
            validate_scene(&template.scene)?;
        }
        Ok(Self { templates, tutorial, mod_entries: BTreeMap::new(), mod_entry_order: Vec::new() })
    }

    /// `createBoneyardCatalog`: a replacement creates fresh entry identities;
    /// cloning this retained catalog shares the same admitted entries.
    pub fn with_mod_entries(mut self, entries: Vec<ModBoneyardEntry>) -> Result<Self, String> {
        let mut by_id = BTreeMap::new();
        let mut order = Vec::new();
        for entry in entries {
            if entry.choice.source != BoneyardSource::Mod {
                return Err("Catalog entries must be mod Boneyards".into());
            }
            let id = entry.choice.id.clone();
            if id == "default-random" || by_id.contains_key(&id) {
                return Err(format!("Duplicate Boneyard id: {id}"));
            }
            order.push(id.clone());
            by_id.insert(id, Arc::new(entry));
        }
        self.mod_entries = by_id;
        self.mod_entry_order = order;
        Ok(self)
    }

    #[must_use]
    pub fn mod_entry(&self, choice_id: &str) -> Option<&Arc<ModBoneyardEntry>> {
        self.mod_entries.get(choice_id)
    }

    pub fn mod_entries(&self) -> impl Iterator<Item = &Arc<ModBoneyardEntry>> {
        self.mod_entry_order.iter().map(|id| &self.mod_entries[id])
    }

    #[must_use]
    pub fn choices(&self) -> Vec<BoneyardChoice> {
        let mut choices = vec![BoneyardChoice {
            id: "default-random".into(),
            name: "Random Boneyard".into(),
            source: BoneyardSource::Default,
            mod_id: None,
            mod_name: None,
        }];
        choices.extend(self.mod_entries().map(|entry| entry.choice.clone()));
        choices
    }

    /// The authoritative host supplies its fresh run identity separately from
    /// the scene-selection seed, as `materializeBoneyard` does in the Website.
    #[must_use]
    pub fn materialize_choice(
        &self,
        choice_id: &str,
        seed: [u8; 16],
        run_id: &str,
    ) -> Option<LoadedBoneyard> {
        if choice_id == "default-random" {
            let mut loaded = self.materialize(seed);
            loaded.run_id = run_id.to_owned();
            return Some(loaded);
        }
        let entry = self.mod_entry(choice_id)?;
        Some(LoadedBoneyard {
            choice_id: entry.choice.id.clone(),
            choice_source: entry.choice.source,
            mod_id: entry.choice.mod_id.clone(),
            mod_name: entry.choice.mod_name.clone(),
            name: entry.choice.name.clone(),
            run_id: run_id.to_owned(),
            seed: seed.iter().map(|byte| format!("{byte:02x}")).collect(),
            source_sha256: entry.source_sha256.clone(),
            geometry_sha256: entry.geometry_sha256.clone(),
            scene: entry.scene.clone(),
        })
    }

    /// Matches `seedBytes.readUInt32BE(0) % NATIVE_GENERATED_BONEYARDS.length`.
    #[must_use]
    pub fn materialize(&self, seed: [u8; 16]) -> LoadedBoneyard {
        let first_word = u32::from_be_bytes([seed[0], seed[1], seed[2], seed[3]]);
        let index = first_word as usize % self.templates.len();
        materialize_template(&self.templates[index], "default-random", "Random Boneyard", seed)
    }

    #[must_use]
    pub fn materialize_tutorial(&self, seed: [u8; 16]) -> LoadedBoneyard {
        materialize_template(&self.tutorial, "stock-tutorial", "Tutorial", seed)
    }
}

pub fn load_boneyard(seed: [u8; 16]) -> Result<LoadedBoneyard, String> {
    Ok(BoneyardCatalog::load_embedded()?.materialize(seed))
}

pub fn load_tutorial(seed: [u8; 16]) -> Result<LoadedBoneyard, String> {
    Ok(BoneyardCatalog::load_embedded()?.materialize_tutorial(seed))
}

/// Reads the same scene document carried by a Website save or Boneyard asset.
pub fn decode_scene(json: &str) -> Result<BoneyardScene, String> {
    let document: SceneDocument =
        serde_json::from_str(json).map_err(|error| format!("Boneyard scene: {error}"))?;
    let scene = document.into();
    validate_scene(&scene)?;
    Ok(scene)
}

pub fn decode_bounds(json: &str) -> Result<BoneyardBounds, String> {
    let document: BoundsDocument =
        serde_json::from_str(json).map_err(|error| format!("Boneyard bounds: {error}"))?;
    Ok(document.into())
}

fn materialize_template(
    template: &BoneyardTemplate,
    choice_id: &str,
    name: &str,
    seed: [u8; 16],
) -> LoadedBoneyard {
    let seed: String = seed.iter().map(|byte| format!("{byte:02x}")).collect();
    LoadedBoneyard {
        choice_id: choice_id.into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: name.into(),
        // The offline caller supplies a fresh 128-bit run seed. It also serves as the local
        // run identity, avoiding a second random draw used only by the multiplayer host.
        run_id: seed.clone(),
        seed,
        source_sha256: template.source_sha256.clone(),
        geometry_sha256: template.geometry_sha256.clone(),
        scene: template.scene.clone(),
    }
}

fn validate_scene(scene: &BoneyardScene) -> Result<(), String> {
    if scene.bounds.width <= 0.0 || scene.bounds.height <= 0.0 {
        return Err(format!("{}: Boneyard bounds must have positive size", scene.name));
    }
    for road in &scene.roads {
        if road.link_mask > 3 {
            return Err(format!("{}: Road {} has an invalid link mask", scene.name, road.eid));
        }
    }
    Ok(())
}

// These documents are the I/O boundary. The simulation uses its own dependency-free domain
// types, so native tests and the browser run exactly the same world and collision code.
#[derive(Deserialize)]
struct PointDocument {
    x: f64,
    y: f64,
}

impl From<PointDocument> for Vec2 {
    fn from(point: PointDocument) -> Self {
        Self::new(point.x, point.y)
    }
}

#[derive(Deserialize)]
struct BoundsDocument {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl From<BoundsDocument> for BoneyardBounds {
    fn from(bounds: BoundsDocument) -> Self {
        Self::new(bounds.x, bounds.y, bounds.w, bounds.h)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpawnDocument {
    x: f64,
    y: f64,
    facing_deg: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ObjectDocument {
    eid: String,
    type_id: u32,
    pos: PointDocument,
    variant: Option<u32>,
    rot: Option<f64>,
    scale: Option<f64>,
    sort_bias: Option<f64>,
    atlas_entry: Option<usize>,
    secondary_atlas_entry: Option<usize>,
    secondary_variant: Option<u32>,
    secondary_visible: Option<bool>,
    overlay_atlas_entry: Option<usize>,
    overlay_variant: Option<u32>,
    atlas_entries: Option<Vec<usize>>,
}

impl From<ObjectDocument> for BoneyardObject {
    fn from(object: ObjectDocument) -> Self {
        Self {
            eid: object.eid,
            type_id: object.type_id,
            position: object.pos.into(),
            variant: object.variant,
            rotation_degrees: object.rot,
            scale: object.scale,
            sort_bias: object.sort_bias,
            atlas_entry: object.atlas_entry,
            secondary_atlas_entry: object.secondary_atlas_entry,
            secondary_variant: object.secondary_variant,
            secondary_visible: object.secondary_visible,
            overlay_atlas_entry: object.overlay_atlas_entry,
            overlay_variant: object.overlay_variant,
            atlas_entries: object.atlas_entries,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpriteDocument {
    eid: String,
    atlas_entry: usize,
    dead_hawg_entry: Option<u64>,
    pos: PointDocument,
    s0: f64,
    s1: f64,
    s2: f64,
    flags: u32,
}

impl From<SpriteDocument> for BoneyardSprite {
    fn from(sprite: SpriteDocument) -> Self {
        Self {
            eid: sprite.eid,
            atlas_entry: sprite.atlas_entry,
            dead_hawg_entry: sprite.dead_hawg_entry,
            position: sprite.pos.into(),
            s0: sprite.s0,
            s1: sprite.s1,
            s2: sprite.s2,
            flags: sprite.flags,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoadDocument {
    eid: String,
    link_mask: u8,
    type_id: u32,
    points: Vec<PointDocument>,
    style: Option<u32>,
    start_width_scale: Option<f64>,
    end_width_scale: Option<f64>,
    quad: Option<Vec<PointDocument>>,
}

impl From<RoadDocument> for BoneyardRoad {
    fn from(road: RoadDocument) -> Self {
        Self {
            eid: road.eid,
            link_mask: road.link_mask,
            type_id: road.type_id,
            points: road.points.into_iter().map(Into::into).collect(),
            style: road.style,
            start_width_scale: road.start_width_scale,
            end_width_scale: road.end_width_scale,
            quad: road.quad.map(|points| points.into_iter().map(Into::into).collect()),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FenceDocument {
    eid: String,
    type_id: u32,
    points: Vec<PointDocument>,
    style: Option<u32>,
    segment_code: Option<u32>,
    start_post_variant: Option<u32>,
    end_post_variant: Option<u32>,
}

impl From<FenceDocument> for BoneyardFence {
    fn from(fence: FenceDocument) -> Self {
        Self {
            eid: fence.eid,
            type_id: fence.type_id,
            points: fence.points.into_iter().map(Into::into).collect(),
            style: fence.style,
            segment_code: fence.segment_code,
            start_post_variant: fence.start_post_variant,
            end_post_variant: fence.end_post_variant,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerrainDocument {
    eid: String,
    pos: PointDocument,
    points: Option<Vec<PointDocument>>,
    style: Option<u32>,
    entry: Option<usize>,
    uid: Option<f64>,
    profile_samples: Option<Vec<f64>>,
    side_sign: Option<f64>,
}

impl From<TerrainDocument> for BoneyardTerrain {
    fn from(terrain: TerrainDocument) -> Self {
        Self {
            eid: terrain.eid,
            position: terrain.pos.into(),
            points: terrain.points.map(|points| points.into_iter().map(Into::into).collect()),
            style: terrain.style,
            entry: terrain.entry,
            uid: terrain.uid,
            profile_samples: terrain.profile_samples,
            side_sign: terrain.side_sign,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SolomonDigDocument {
    grave_position: PointDocument,
    lantern_position: PointDocument,
    position: PointDocument,
    frame_program: Vec<usize>,
    ticks_per_frame: u64,
}

impl From<SolomonDigDocument> for SolomonDig {
    fn from(dig: SolomonDigDocument) -> Self {
        Self {
            grave_position: dig.grave_position.into(),
            lantern_position: dig.lantern_position.into(),
            position: dig.position.into(),
            frame_program: dig.frame_program,
            ticks_per_frame: dig.ticks_per_frame,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneDocument {
    name: String,
    environment_mode: u32,
    bounds: BoundsDocument,
    spawn: SpawnDocument,
    objects: Vec<ObjectDocument>,
    sprites: Vec<SpriteDocument>,
    roads: Vec<RoadDocument>,
    fences: Vec<FenceDocument>,
    terrain: Vec<TerrainDocument>,
    solomon_dig: Option<SolomonDigDocument>,
}

impl From<SceneDocument> for BoneyardScene {
    fn from(scene: SceneDocument) -> Self {
        Self {
            name: scene.name,
            environment_mode: scene.environment_mode,
            bounds: scene.bounds.into(),
            spawn: BoneyardSpawn {
                position: Vec2::new(scene.spawn.x, scene.spawn.y),
                facing_degrees: scene.spawn.facing_deg,
            },
            objects: scene.objects.into_iter().map(Into::into).collect(),
            sprites: scene.sprites.into_iter().map(Into::into).collect(),
            roads: scene.roads.into_iter().map(Into::into).collect(),
            fences: scene.fences.into_iter().map(Into::into).collect(),
            terrain: scene.terrain.into_iter().map(Into::into).collect(),
            solomon_dig: scene.solomon_dig.map(Into::into),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemplateDocument {
    source_sha256: String,
    geometry_sha256: String,
    scene: SceneDocument,
}

impl From<TemplateDocument> for BoneyardTemplate {
    fn from(template: TemplateDocument) -> Self {
        Self {
            source_sha256: template.source_sha256,
            geometry_sha256: template.geometry_sha256,
            scene: template.scene.into(),
        }
    }
}

#[derive(Deserialize)]
struct CatalogDocument {
    templates: Vec<TemplateDocument>,
    tutorial: TemplateDocument,
}
