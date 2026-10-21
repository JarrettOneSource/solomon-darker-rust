//! Immutable original glyph records and recovered shape tables. Produced by
//! `tools/export-boneyard-catalog.mjs`; no scene placements are baked here.

use serde::Deserialize;
use std::collections::BTreeMap;

use super::{BoundsDocument, NativeBoundsDocument, PointDocument};
use solomon_sim::Vec2;
use solomon_sim::boneyard::scenery::native_scenery_art_corners;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SceneryCatalog {
    pub records: Vec<Option<SceneryRecord>>,
    pub shadows: BTreeMap<String, Vec<Vec<PointDocument>>>,
    pub tree_bounds: Vec<NativeBoundsDocument>,
    pub tree_polygons: Vec<Vec<PointDocument>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SceneryRecord {
    pub file: String,
    pub w: f64,
    pub h: f64,
    pub anchor_x: f64,
    pub anchor_y: f64,
}

impl SceneryCatalog {
    pub fn record(&self, entry: usize) -> Option<&SceneryRecord> {
        self.records.get(entry).and_then(Option::as_ref)
    }

    pub fn outline(&self, family: &str, variant: usize) -> Result<Vec<PointDocument>, String> {
        self.shadows
            .get(family)
            .and_then(|rows| rows.get(variant))
            .cloned()
            .ok_or_else(|| format!("Unsupported native {family} complex-shadow variant {variant}"))
    }
}

impl SceneryRecord {
    pub fn shadow_glyph(&self) -> super::NativeSceneryShadowGlyph {
        super::NativeSceneryShadowGlyph {
            width: self.w,
            height: self.h,
            anchor_x: self.anchor_x,
            anchor_y: self.anchor_y,
        }
    }

    pub fn corners(&self, position: Vec2, rotation: f64, scale: Vec2) -> [Vec2; 4] {
        native_scenery_art_corners(
            position,
            Vec2::new(self.w, self.h),
            Vec2::new(self.anchor_x, self.anchor_y),
            rotation,
            scale,
        )
    }

    pub fn bounds(&self, position: Vec2, rotation: f64, scale: Vec2) -> BoundsDocument {
        BoundsDocument::points(&self.corners(position, rotation, scale))
    }
}
