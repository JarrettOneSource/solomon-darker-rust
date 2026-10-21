//! World-light collection from `core-server/boneyard-world-light.ts`.
//!
//! Providers keep their native manager registrations. Each participant contributes a stock
//! view, including participants who are not currently alive; logical spawn queries use the
//! maximum of those exact manager fields, rather than a renderer's visibility selection.

mod entities;

pub use entities::{
    BoneyardEntityLightPresentation, BoneyardLightEnvironment, boneyard_world_light_query,
    collect_boneyard_entity_lights,
};

use crate::Vec2;
use crate::float::fround;
use crate::painter::{PainterRegistration, WorldManagerLane};
pub use crate::region_feedback::native_region_point_gain;

use super::BoneyardBounds;
use super::lighting::{
    NATIVE_DEFAULT_LIGHT_QUALITY, NativeBoneyardLightIndex, NativeBoneyardLightManagerView,
    NativeBoneyardLightSource, NativePlayerLightOwner, native_lantern_light_source,
    native_player_light_source,
};

const WORLD_LIGHT_ZOOM: f64 = 1.35;
const STOCK_VIEWPORT: Vec2 = Vec2::new(1600.0, 900.0);

#[derive(Clone, Copy, Debug)]
pub struct WorldLightPlayer<'a> {
    pub id: &'a str,
    pub position: Vec2,
    pub heading_index: usize,
    pub weapon_pulse: f64,
    pub overlay_effect_phase: f64,
    pub light_registration: Option<PainterRegistration>,
    pub viewport: Option<Vec2>,
}

/// One actual actor or transient may submit several sources under a single registration.
/// Entity-specific source factories remain in `lighting`; collection never invents a source.
#[derive(Clone, Debug)]
pub struct WorldLightOwner {
    pub registration: PainterRegistration,
    pub sources: Vec<NativeBoneyardLightSource>,
}

#[derive(Clone, Debug)]
pub struct WorldMiscLightOwner {
    pub birth_tick: i64,
    pub id: usize,
    pub append_ordinal: usize,
    pub owner: WorldLightOwner,
}

#[derive(Clone, Copy, Debug)]
pub struct WorldLightScene {
    pub bounds: BoneyardBounds,
    pub camera_bounds: BoneyardBounds,
    pub lantern_position: Option<Vec2>,
    pub lantern_light_registration: Option<PainterRegistration>,
}

#[derive(Clone, Debug)]
pub struct BoneyardWorldLightQuery {
    pub cameras: Vec<BoneyardBounds>,
    indices: Vec<NativeBoneyardLightIndex>,
}

impl BoneyardWorldLightQuery {
    #[must_use]
    pub fn new(
        scene: WorldLightScene,
        players: &[WorldLightPlayer<'_>],
        provider_owners: &[WorldLightOwner],
        misc_owners: &[WorldMiscLightOwner],
        tick: u64,
    ) -> Self {
        let mut cameras = Vec::with_capacity(players.len());
        let mut providers = Vec::with_capacity(players.len() + provider_owners.len() + 1);
        for (index, player) in players.iter().enumerate() {
            cameras.push(native_primary_view_bounds(
                scene.camera_bounds,
                player.position,
                player.viewport.unwrap_or(STOCK_VIEWPORT),
                WORLD_LIGHT_ZOOM,
                0.0,
            ));
            let source = native_player_light_source(
                NativePlayerLightOwner {
                    id: player.id,
                    position: player.position,
                    heading_index: player.heading_index,
                    drive_active: false,
                    overlay_effect_phase: player.weapon_pulse.max(player.overlay_effect_phase),
                },
                tick,
                true,
            )
            .expect("local player light always submits a source");
            providers.push(WorldLightOwner {
                registration: player.light_registration.unwrap_or(PainterRegistration {
                    manager_lane: WorldManagerLane::Actor,
                    registration_ordinal: index + 1,
                }),
                sources: vec![source],
            });
        }
        providers.extend_from_slice(provider_owners);
        if let Some(position) = scene.lantern_position {
            providers.push(WorldLightOwner {
                registration: scene
                    .lantern_light_registration
                    .expect("lantern light has a native manager registration"),
                sources: vec![native_lantern_light_source(position, tick, true)],
            });
        }
        let mut misc = misc_owners.to_vec();
        let (sources, tail) = ordered_boneyard_light_sources(&mut providers, &mut misc);

        let mut indices = Vec::with_capacity(cameras.len());
        for camera in &cameras {
            let mut index =
                NativeBoneyardLightIndex::new(Vec2::new(scene.bounds.width, scene.bounds.height));
            index.rebuild(
                &sources,
                &tail,
                NativeBoneyardLightManagerView {
                    camera_position: Vec2::new(
                        camera.x + camera.width / 2.0,
                        camera.y + camera.height / 2.0,
                    ),
                    zoom: WORLD_LIGHT_ZOOM,
                    viewport: Vec2::new(
                        camera.width * WORLD_LIGHT_ZOOM,
                        camera.height * WORLD_LIGHT_ZOOM,
                    ),
                },
                NATIVE_DEFAULT_LIGHT_QUALITY,
            );
            indices.push(index);
        }
        Self { cameras, indices }
    }

    #[must_use]
    pub fn scalar_at(&self, position: Vec2) -> f64 {
        let mut scalar: f64 = 0.0;
        for index in &self.indices {
            scalar = scalar.max(index.scalar_at(position));
        }
        scalar
    }

    /// The source exposes concatenated accepted providers, including one copy per camera.
    pub fn accepted_sources(&self) -> impl Iterator<Item = &NativeBoneyardLightSource> {
        self.indices.iter().flat_map(|index| index.accepted_sources())
    }

    #[must_use]
    pub fn point_gain_at(&self, position: Vec2) -> f64 {
        let mut gain: f64 = 0.0;
        for camera in &self.cameras {
            let center = Vec2::new(camera.x + camera.width / 2.0, camera.y + camera.height / 2.0);
            gain = gain.max(native_region_point_gain(position, center, camera.width, false));
        }
        gain
    }
}

/// `primary-spell-targeting.ts::nativePrimaryViewBounds`, including each float32 store.
#[must_use]
pub fn native_primary_view_bounds(
    bounds: BoneyardBounds,
    focus: Vec2,
    viewport: Vec2,
    scale: f64,
    padding: f64,
) -> BoneyardBounds {
    let width = fround(viewport.x / scale);
    let height = fround(viewport.y / scale);
    let camera_x = clamp_view_axis(focus.x, bounds.x, bounds.width, width);
    let camera_y = clamp_view_axis(focus.y, bounds.y, bounds.height, height);
    BoneyardBounds::new(
        fround(camera_x - width / 2.0 - padding),
        fround(camera_y - height / 2.0 - padding),
        fround(width + padding * 2.0),
        fround(height + padding * 2.0),
    )
}

/// Provider registration order and native MiscLight append order are shared by logical
/// queries and the renderer; camera filtering happens only after this complete sequence.
#[must_use]
pub fn ordered_boneyard_light_sources(
    providers: &mut [WorldLightOwner],
    misc: &mut [WorldMiscLightOwner],
) -> (Vec<NativeBoneyardLightSource>, Vec<NativeBoneyardLightSource>) {
    let sources = ordered_provider_sources(providers);
    // The first stable sort only breaks ties inside the same manager registration.
    misc.sort_by_key(|entry| (entry.append_ordinal, entry.birth_tick, entry.id));
    let mut misc: Vec<_> = misc.iter().map(|entry| entry.owner.clone()).collect();
    (sources, ordered_provider_sources(&mut misc))
}

fn ordered_provider_sources(owners: &mut [WorldLightOwner]) -> Vec<NativeBoneyardLightSource> {
    owners.sort_by_key(|owner| {
        let lane = match owner.registration.manager_lane {
            WorldManagerLane::Actor => 0,
            WorldManagerLane::Transient => 1,
        };
        (lane, owner.registration.registration_ordinal)
    });
    let mut sources = Vec::new();
    for owner in owners {
        sources.extend_from_slice(&owner.sources);
    }
    sources
}

fn clamp_view_axis(focus: f64, origin: f64, size: f64, view_size: f64) -> f64 {
    if size <= view_size {
        return fround(origin + size / 2.0);
    }
    fround(focus.max(origin + view_size / 2.0).min(origin + size - view_size / 2.0))
}
