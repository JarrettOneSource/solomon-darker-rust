//! Shared logical and visual light rules from `native-boneyard-lighting.ts` and
//! `native-boneyard-light-model.ts`. Enemy placement and drawing query the same field.

pub mod enemy;
pub mod primary;
pub mod query;
pub mod secondary;
pub mod weld;

pub use primary::*;
pub use query::{
    NativeBoneyardLightFactors, NativeBoneyardLightQuery, native_boneyard_source_light_factors,
};

use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::random_domain::{native_random_float_from_word, native_random_int_from_word};

pub const NATIVE_LIGHT_INNER_DISTANCE: f64 = 75.0;
pub const NATIVE_LIGHT_OUTER_DISTANCE: f64 = 145.0;
pub const NATIVE_LIGHT_VERTICAL_SCALE: f64 = 0.85_f32 as f64;
pub const NATIVE_PLAYER_LIGHT_RADIUS: f64 = 2.6_f32 as f64;
pub const NATIVE_PLAYER_LIGHT_OFFSET: f64 = 15.0;
pub const NATIVE_DEFAULT_LIGHT_QUALITY: f64 = 0.25;
const LIGHT_GRID_CELL_SIZE: f64 = 150.0;
const LIGHT_WORLD_SCALE: f64 = 0.8;
const LIGHT_BOTTOM_PADDING: f64 = 350.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBoneyardLightSource {
    pub intensity: f64,
    pub position: Vec2,
    pub radius: f64,
    pub casts_directional_shadow: bool,
    pub raster_scale: Option<f64>,
}

impl NativeBoneyardLightSource {
    #[must_use]
    pub const fn new(position: Vec2, radius: f64, intensity: f64, casts_shadow: bool) -> Self {
        Self {
            intensity,
            position,
            radius,
            casts_directional_shadow: casts_shadow,
            raster_scale: None,
        }
    }

    /// The world manager stores single-precision values when accepting a provider.
    fn submitted(self) -> Self {
        Self {
            intensity: fround(self.intensity),
            position: Vec2::new(fround(self.position.x), fround(self.position.y)),
            radius: fround(self.radius),
            raster_scale: self.raster_scale.map(fround),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativePlayerLightOwner<'a> {
    pub id: &'a str,
    pub position: Vec2,
    pub heading_index: usize,
    pub drive_active: bool,
    pub overlay_effect_phase: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBoneyardLightManagerView {
    pub camera_position: Vec2,
    pub zoom: f64,
    pub viewport: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeRegionLightTargetPlan {
    pub logical_side: f64,
    pub physical_side: u32,
    pub render_resolution: f64,
}

#[must_use]
pub fn native_region_light_target_plan(
    viewport: Vec2,
    device_resolution: f64,
    quality: f64,
) -> NativeRegionLightTargetPlan {
    let requested = (viewport.x.max(viewport.y) * device_resolution * quality).trunc().max(1.0);
    let physical_side = (requested as u32).next_power_of_two();
    let render_resolution =
        fround(fround(device_resolution) * fround(fround(quality) * fround(LIGHT_WORLD_SCALE)));
    NativeRegionLightTargetPlan {
        logical_side: f64::from(physical_side) / render_resolution,
        physical_side,
        render_resolution,
    }
}

/// A bounded native grid. Buckets keep their allocation between rebuilds, and source
/// indices retain manager submission order. No scene query depends on map iteration order.
#[derive(Clone, Debug)]
pub struct NativeBoneyardLightIndex {
    accepted: Vec<NativeBoneyardLightSource>,
    buckets: Vec<Vec<usize>>,
    maximum_column: i32,
    maximum_row: i32,
    row_count: usize,
}

impl NativeBoneyardLightIndex {
    #[must_use]
    pub fn new(extent: Vec2) -> Self {
        let maximum_column =
            fround(fround(extent.x) / fround(LIGHT_GRID_CELL_SIZE)).ceil() as i32 + 1;
        let maximum_row = fround(fround(extent.y) / fround(LIGHT_GRID_CELL_SIZE)).ceil() as i32 + 1;
        let row_count = (maximum_row + 3) as usize;
        Self {
            accepted: Vec::new(),
            buckets: vec![Vec::new(); (maximum_column + 3) as usize * row_count],
            maximum_column,
            maximum_row,
            row_count,
        }
    }

    pub fn rebuild(
        &mut self,
        providers: &[NativeBoneyardLightSource],
        misc_tail: &[NativeBoneyardLightSource],
        view: NativeBoneyardLightManagerView,
        quality: f64,
    ) {
        self.accepted.clear();
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        for source in providers.iter().chain(misc_tail) {
            let candidate = source.submitted();
            if !native_boneyard_light_visible_in_manager(candidate, view, quality) {
                continue;
            }
            if !candidate.casts_directional_shadow && self.candidate_contained(candidate) {
                continue;
            }
            let index = self.accepted.len();
            self.accepted.push(candidate);
            self.index(candidate, index);
        }
    }

    #[must_use]
    pub fn accepted_sources(&self) -> &[NativeBoneyardLightSource] {
        &self.accepted
    }

    #[must_use]
    pub fn source_indices_at(&self, position: Vec2) -> &[usize] {
        let x = grid_coordinate(position.x);
        let y = grid_coordinate(position.y);
        if x < -2 || y < -2 || x > self.maximum_column || y > self.maximum_row {
            return &[];
        }
        &self.buckets[self.bucket_index(x, y)]
    }

    #[must_use]
    pub fn scalar_at(&self, position: Vec2) -> f64 {
        self.factors_at(position, NativeBoneyardLightQuery::default()).radial
    }

    #[must_use]
    pub fn factors_at(
        &self,
        position: Vec2,
        query: NativeBoneyardLightQuery,
    ) -> NativeBoneyardLightFactors {
        query::factors(
            position,
            self.source_indices_at(position).iter().map(|&index| self.accepted[index]),
            query,
        )
    }

    #[must_use]
    pub fn surface_scalar_at(&self, position: Vec2) -> f64 {
        self.surface_scalar_with_query(position, NativeBoneyardLightQuery::default())
    }

    #[must_use]
    pub fn surface_scalar_with_query(
        &self,
        position: Vec2,
        query: NativeBoneyardLightQuery,
    ) -> f64 {
        query::surface(
            position,
            self.source_indices_at(position).iter().map(|&index| self.accepted[index]),
            query,
        )
    }

    fn candidate_contained(&self, candidate: NativeBoneyardLightSource) -> bool {
        for &index in self.source_indices_at(candidate.position) {
            let existing = self.accepted[index];
            if existing.intensity < candidate.intensity || existing.radius < candidate.radius {
                continue;
            }
            let dx = existing.position.x - candidate.position.x;
            let dy = existing.position.y - candidate.position.y;
            let reach = (existing.radius - candidate.radius) * NATIVE_LIGHT_OUTER_DISTANCE;
            if dx * dx + dy * dy < reach * reach {
                return true;
            }
        }
        false
    }

    fn index(&mut self, source: NativeBoneyardLightSource, source_index: usize) {
        let reach = fround(fround(source.radius) * fround(NATIVE_LIGHT_OUTER_DISTANCE));
        let min_x = grid_coordinate(fround(fround(source.position.x) - reach))
            .clamp(-2, self.maximum_column);
        let max_x = grid_coordinate(fround(fround(source.position.x) + reach))
            .clamp(-2, self.maximum_column);
        let min_y =
            grid_coordinate(fround(fround(source.position.y) - reach)).clamp(-2, self.maximum_row);
        let max_y =
            grid_coordinate(fround(fround(source.position.y) + reach)).clamp(-2, self.maximum_row);
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let bucket = self.bucket_index(x, y);
                self.buckets[bucket].push(source_index);
            }
        }
    }

    fn bucket_index(&self, x: i32, y: i32) -> usize {
        (x + 2) as usize * self.row_count + (y + 2) as usize
    }
}

#[must_use]
pub fn native_boneyard_radial_light_contribution(
    position: Vec2,
    source: NativeBoneyardLightSource,
) -> f64 {
    native_boneyard_source_light_factors(position, source, NativeBoneyardLightQuery::default())
        .radial
}

#[must_use]
pub fn native_boneyard_light_scalar(position: Vec2, sources: &[NativeBoneyardLightSource]) -> f64 {
    query::factors(position, sources.iter().copied(), NativeBoneyardLightQuery::default()).radial
}

#[must_use]
pub fn native_boneyard_light_factors(
    position: Vec2,
    sources: &[NativeBoneyardLightSource],
    query: NativeBoneyardLightQuery,
) -> NativeBoneyardLightFactors {
    query::factors(position, sources.iter().copied(), query)
}

#[must_use]
pub fn native_boneyard_surface_light_scalar_with_query(
    position: Vec2,
    sources: &[NativeBoneyardLightSource],
    query: NativeBoneyardLightQuery,
) -> f64 {
    query::surface(position, sources.iter().copied(), query)
}

#[must_use]
pub fn native_boneyard_surface_light_scalar(
    position: Vec2,
    sources: &[NativeBoneyardLightSource],
) -> f64 {
    query::surface(position, sources.iter().copied(), NativeBoneyardLightQuery::default())
}

#[must_use]
pub fn native_boneyard_light_tint(scalar: f64) -> u32 {
    let lane = fround(fround(scalar.clamp(0.0, 1.0)) * 255.0).trunc() as u32;
    lane * 0x010101
}

#[must_use]
pub fn native_boneyard_light_visible_in_manager(
    source: NativeBoneyardLightSource,
    view: NativeBoneyardLightManagerView,
    quality: f64,
) -> bool {
    let source = source.submitted();
    let manager_scale = fround(fround(quality) * fround(LIGHT_WORLD_SCALE));
    let target_width = fround(fround(view.viewport.x / view.zoom) * fround(quality));
    // The native target is a width-derived square with bottom padding, not a viewport ratio.
    let target_height =
        fround(target_width + fround(fround(quality) * fround(LIGHT_BOTTOM_PADDING)));
    let top_left = Vec2::new(
        fround(view.camera_position.x - view.viewport.x / (2.0 * view.zoom)),
        fround(view.camera_position.y - view.viewport.y / (2.0 * view.zoom)),
    );
    let x = fround(fround(fround(source.position.x) - top_left.x) * manager_scale);
    let y = fround(fround(fround(source.position.y) - top_left.y) * manager_scale);
    let reach =
        fround(fround(fround(source.radius) * fround(NATIVE_LIGHT_OUTER_DISTANCE)) * manager_scale);
    let dx = fround(x - x.clamp(0.0, target_width));
    let dy = fround(y - y.clamp(0.0, target_height));
    fround(dx * dx + dy * dy) < fround(reach * reach)
}

#[must_use]
pub fn native_player_light_source(
    player: NativePlayerLightOwner<'_>,
    frame: u64,
    is_local: bool,
) -> Option<NativeBoneyardLightSource> {
    if player.drive_active && !is_local {
        return None;
    }
    let heading = player.heading_index as f64 * 15.0 * std::f64::consts::PI / 180.0;
    let jitter = presentation_random(frame, stable_string_hash(player.id) ^ 0x5299a0, 0.2);
    Some(NativeBoneyardLightSource {
        intensity: 1.0,
        casts_directional_shadow: true,
        position: Vec2::new(
            player.position.x + float::sin(heading) * NATIVE_PLAYER_LIGHT_OFFSET,
            player.position.y - float::cos(heading) * NATIVE_PLAYER_LIGHT_OFFSET,
        ),
        radius: (1.0 + player.overlay_effect_phase) * NATIVE_PLAYER_LIGHT_RADIUS,
        raster_scale: Some(fround(fround(NATIVE_PLAYER_LIGHT_RADIUS) - jitter)),
    })
}

#[must_use]
pub fn native_lantern_light_source(
    position: Vec2,
    frame: u64,
    multiple_shadows: bool,
) -> NativeBoneyardLightSource {
    let intensity = fround(fround(0.55) + presentation_signed_random(frame, 0, 0.2));
    NativeBoneyardLightSource::new(position, 0.65, intensity, multiple_shadows)
}

#[must_use]
pub fn native_missile_light_source(
    id: u32,
    position: Vec2,
    frame: u64,
    multiple_shadows: bool,
) -> NativeBoneyardLightSource {
    let radius = fround(fround(0.75) + presentation_signed_random(frame, id ^ 0x5e4af0, 0.1));
    NativeBoneyardLightSource::new(position, radius, 0.75, multiple_shadows)
}

#[must_use]
pub fn native_boulder_light_source(
    position: Vec2,
    charge: f64,
    multiple_shadows: bool,
) -> NativeBoneyardLightSource {
    NativeBoneyardLightSource::new(position, (2.0 * charge).max(1.0), 0.5, multiple_shadows)
}

#[must_use]
pub fn native_fire_patch_light_source(
    position: Vec2,
    life: f64,
    multiple_shadows: bool,
) -> NativeBoneyardLightSource {
    NativeBoneyardLightSource::new(
        position,
        0.6_f32 as f64,
        (3.0 * life).min(1.0),
        multiple_shadows,
    )
}

#[must_use]
pub fn presentation_random_word(frame: u64, salt: u32) -> u32 {
    let mut value = frame as u32 ^ salt ^ 0x9e37_79b9;
    value = (value ^ (value >> 16)).wrapping_mul(0x21f0_aaad);
    value = (value ^ (value >> 15)).wrapping_mul(0x735a_2d97);
    value ^ (value >> 15)
}

#[must_use]
pub fn presentation_random(frame: u64, salt: u32, maximum: f64) -> f64 {
    native_random_float_from_word(presentation_random_word(frame, salt), maximum)
}

#[must_use]
pub fn presentation_signed_random(frame: u64, salt: u32, maximum: f64) -> f64 {
    let magnitude = presentation_random(frame, salt, maximum);
    let sign = native_random_int_from_word(presentation_random_word(frame, salt ^ 0x7f4a_7c15), 2);
    if sign == 1 { fround(-magnitude) } else { magnitude }
}

/// Raw UTF-16 FNV hash; presentation and tree countdowns retain zero as a valid result.
#[must_use]
pub fn stable_string_hash(value: &str) -> u32 {
    let mut hash = 0x811c_9dc5;
    for unit in value.encode_utf16() {
        hash = (hash ^ u32::from(unit)).wrapping_mul(0x0100_0193);
    }
    hash
}

fn grid_coordinate(value: f64) -> i32 {
    fround(fround(value) / fround(LIGHT_GRID_CELL_SIZE)).trunc() as i32
}
