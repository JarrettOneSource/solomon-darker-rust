//! The courtyard's stock art, placed the way the retail `Courtyard::Present` submits it.
//!
//! Every College record is registered inside a 2000×1000 logical frame, so drawing each one at
//! its registered origin reproduces the native placement without a flattened atlas. The order
//! and banks mirror the Website's `tools/hub_room_art.py` and `hub-world-scene.ts`.

use solomon_assets::SpriteRecord;
use solomon_sim::Vec2;
use solomon_sim::hub::ambient::{
    HUB_FOUNTAIN_ORIGIN, HubAmbientState, HubColor, HubStatueOffsets, hub_fountain_particle_alpha,
    hub_seal_colors, hub_statue_offsets,
};
use solomon_sim::hub::depth::{HUB_WORLD_DEPTH, HubDepth};
use solomon_sim::hub::painter::{
    HUB_COLLEGE_STATUE, HUB_COLLEGE_STATUE_PAINTER_ORDINAL,
    HUB_COURTYARD_OBSTACLE_PAINTER_ORDINAL_BASE, HUB_COURTYARD_OBSTACLES, PainterLane,
    PainterLayer, hub_painter_order,
};
use solomon_sim::hub::{HUB_CAMERA_SCALE, HUB_LOGICAL_VIEW, HUB_WORLD_SIZE};
use solomon_sim::painter::overlays::{PainterOverlayOrder, PainterOwnerOverlay};
use std::collections::BTreeMap;

use solomon_graphics::{BlendMode, Quad, Renderer, Texture};

pub struct Camera {
    pub origin: Vec2,
    pub scale: f64,
}

impl Camera {
    /// Logical-pixel rectangle of a world rectangle.
    pub fn project(&self, x: f64, y: f64, width: f64, height: f64) -> [f32; 4] {
        [
            ((x - self.origin.x) * self.scale) as f32,
            ((y - self.origin.y) * self.scale) as f32,
            (width * self.scale) as f32,
            (height * self.scale) as f32,
        ]
    }
}

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const FLOOR_TILE: usize = 42;
const BASE_SCENERY: [usize; 3] = [2, 6, 26];
/// The runic ring under the lower-right courtyard: drawn at (1500, 1000) in RGBA (1, 1, .5, .25).
const RUNIC_RING: usize = 13;
const RUNIC_RING_OFFSET: Vec2 = Vec2::new(1500.0, 1000.0);
const RUNIC_RING_TINT: [f32; 4] = [1.0, 1.0, 0.5, 0.25];
const UPPER_BANK: std::ops::Range<usize> = 63..89;
const LOWER_BANK: std::ops::Range<usize> = 93..106;
/// Submitted after every actor.
const FOREGROUND_BANK: [usize; 5] = [19, 30, 31, 21, 22];
const STATUE_BODY: usize = 39;
/// Multiplied over the floor just before the body.
const STATUE_AURA: usize = 41;
/// The seal's glyph masks and its core: `Courtyard::Present` adds them over the floor at twice
/// their registered size, tinted by the ambient pulse.
const SEAL_GLYPHS: std::ops::Range<usize> = 106..119;
const SEAL_CORE: usize = 12;
const SEAL_SCALE: f64 = 2.0;
/// One drop of fountain spray, added from its centre at the particle's scale.
const FOUNTAIN_PARTICLE: usize = 38;

/// The art of each courtyard obstacle, in step with `HUB_COURTYARD_OBSTACLES`.
const OBSTACLE_RECORDS: [&[usize]; HUB_COURTYARD_OBSTACLES.len()] = [
    &[148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159],
    &[25],
    &[23],
    &[28],
    &[29],
    &[27],
    &[20],
    &[24],
];

/// Caller-owned actors and fixed bands in the courtyard's single depth-sorted submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CourtyardPass {
    Actor(usize),
    Band(HubDepth),
}

/// Canonical actor rows with the view metadata needed by the final overlay pass.
pub struct ActorPainters<'a> {
    pub layers: &'a [PainterLayer],
    pub owner_overlays: &'a [PainterOwnerOverlay],
    pub live_views: &'a [(u64, u64)],
}

#[derive(Clone, Copy)]
enum CourtyardLayer {
    Band(HubDepth),
    Obstacle(usize),
    Statue,
    Actor(usize),
}

pub struct CourtyardScene {
    records: Vec<SpriteRecord>,
    atlas: Texture,
    overlay_order: PainterOverlayOrder,
    southern: Vec<(usize, Vec2)>,
}

impl CourtyardScene {
    pub fn new(records: Vec<SpriteRecord>, atlas: Texture) -> Result<Self, String> {
        let highest = LOWER_BANK.end.max(SEAL_GLYPHS.end).max(
            OBSTACLE_RECORDS.iter().flat_map(|records| records.iter()).copied().max().unwrap_or(0)
                + 1,
        );
        if records.len() < highest {
            return Err(format!(
                "College.bundle has {} records; the courtyard needs {highest}",
                records.len()
            ));
        }
        Ok(Self {
            records,
            atlas,
            overlay_order: PainterOverlayOrder::default(),
            southern: southern_architecture(),
        })
    }

    pub fn reset_overlays(&mut self) {
        self.overlay_order.clear();
    }

    /// Fixed bands and world actors share one submission order. Rows sort actors, and the
    /// Website planner assigns them consecutive depths after 1000; a northern row therefore
    /// cannot accidentally move an actor under the fountain's fixed depth 980.
    pub fn draw(
        &mut self,
        renderer: &mut Renderer,
        camera: &Camera,
        ambient: &HubAmbientState,
        reference_y: f64,
        actors: &ActorPainters<'_>,
        mut draw: impl FnMut(&mut Renderer, CourtyardPass),
    ) {
        let mut layers = Vec::with_capacity(HUB_WORLD_DEPTH.len() + actors.layers.len() + 9);
        for band in HUB_WORLD_DEPTH {
            // These two initial depths are replaced by the statue's active world painter.
            if matches!(band, HubDepth::StatueAura | HubDepth::Statue) {
                continue;
            }
            layers.push((band.value(), CourtyardLayer::Band(band)));
        }
        let mut painters =
            Vec::with_capacity(HUB_COURTYARD_OBSTACLES.len() + 1 + actors.layers.len());
        let mut world_layers = Vec::with_capacity(painters.capacity());
        for (index, obstacle) in HUB_COURTYARD_OBSTACLES.iter().enumerate() {
            painters.push(PainterLayer {
                visible: true,
                is_insertion: false,
                manager_lane: PainterLane::Actor,
                registration_ordinal: HUB_COURTYARD_OBSTACLE_PAINTER_ORDINAL_BASE + index,
                world_y: obstacle.position.y,
                sort_bias: obstacle.sort_bias,
            });
            world_layers.push(CourtyardLayer::Obstacle(index));
        }
        painters.push(PainterLayer {
            visible: true,
            is_insertion: false,
            manager_lane: PainterLane::Actor,
            registration_ordinal: HUB_COLLEGE_STATUE_PAINTER_ORDINAL,
            world_y: HUB_COLLEGE_STATUE.position.y,
            sort_bias: HUB_COLLEGE_STATUE.sort_bias,
        });
        world_layers.push(CourtyardLayer::Statue);
        painters.extend_from_slice(actors.layers);
        for index in 0..actors.layers.len() {
            world_layers.push(CourtyardLayer::Actor(index));
        }
        let mut actor_draw_ids = BTreeMap::new();
        for (rank, index) in hub_painter_order(&painters, reference_y).into_iter().enumerate() {
            if let CourtyardLayer::Actor(actor) = world_layers[index] {
                actor_draw_ids.insert(actor, layers.len());
            }
            layers.push((1001.0 + rank as f64, world_layers[index]));
        }
        let positioned: Vec<_> =
            layers.iter().enumerate().map(|(id, (depth, _))| (id, *depth)).collect();
        let overlays: Vec<_> = actors
            .owner_overlays
            .iter()
            .filter_map(|overlay| {
                Some(PainterOwnerOverlay {
                    draw_id: *actor_draw_ids.get(&overlay.draw_id)?,
                    owner_draw_id: actor_draw_ids
                        .get(&overlay.owner_draw_id)
                        .copied()
                        .unwrap_or(usize::MAX),
                    view_id: overlay.view_id,
                })
            })
            .collect();
        let order = self.overlay_order.apply(&positioned, &overlays, actors.live_views);
        let colors = hub_seal_colors(ambient);
        let statue = hub_statue_offsets(ambient);
        for draw_id in order {
            match layers[draw_id].1 {
                CourtyardLayer::Band(HubDepth::Courtyard) => self.draw_background(renderer, camera),
                CourtyardLayer::Band(HubDepth::SealGlyphs) => {
                    for index in SEAL_GLYPHS {
                        self.draw_scaled_additive(
                            renderer,
                            camera,
                            index,
                            SEAL_SCALE,
                            tint(colors.glyphs),
                        );
                    }
                }
                CourtyardLayer::Band(HubDepth::SealCore) => {
                    self.draw_scaled_additive(
                        renderer,
                        camera,
                        SEAL_CORE,
                        SEAL_SCALE,
                        tint(colors.core),
                    );
                }
                CourtyardLayer::Band(HubDepth::Fountain) => {
                    self.draw_fountain(renderer, camera, ambient)
                }
                CourtyardLayer::Band(HubDepth::CourtyardForeground) => {
                    for index in FOREGROUND_BANK {
                        self.draw_registered(renderer, camera, index, Vec2::ZERO, WHITE);
                    }
                }
                CourtyardLayer::Band(HubDepth::SouthernForeground) => {
                    self.draw_southern(renderer, camera);
                }
                CourtyardLayer::Band(band) => draw(renderer, CourtyardPass::Band(band)),
                CourtyardLayer::Obstacle(obstacle) => {
                    for &index in OBSTACLE_RECORDS[obstacle] {
                        self.draw_registered(renderer, camera, index, Vec2::ZERO, WHITE);
                    }
                }
                CourtyardLayer::Statue => self.draw_statue(renderer, camera, statue),
                CourtyardLayer::Actor(index) => draw(renderer, CourtyardPass::Actor(index)),
            }
        }
    }

    fn draw_background(&self, renderer: &mut Renderer, camera: &Camera) {
        self.draw_floor(renderer, camera);
        for index in BASE_SCENERY {
            self.draw_registered(renderer, camera, index, Vec2::ZERO, WHITE);
        }
        self.draw_registered(renderer, camera, RUNIC_RING, RUNIC_RING_OFFSET, RUNIC_RING_TINT);
        for index in UPPER_BANK.chain(LOWER_BANK) {
            self.draw_registered(renderer, camera, index, Vec2::ZERO, WHITE);
        }
    }

    fn draw_fountain(&self, renderer: &mut Renderer, camera: &Camera, ambient: &HubAmbientState) {
        let drop = &self.records[FOUNTAIN_PARTICLE];
        for particle in &ambient.fountain_particles {
            let width = f64::from(drop.width) * particle.scale;
            let height = f64::from(drop.height) * particle.scale;
            renderer.draw_clamped(
                &self.atlas,
                Quad {
                    source: atlas_source(drop),
                    destination: camera.project(
                        HUB_FOUNTAIN_ORIGIN.x - width / 2.0,
                        HUB_FOUNTAIN_ORIGIN.y - height / 2.0,
                        width,
                        height,
                    ),
                    tint: [1.0, 1.0, 1.0, hub_fountain_particle_alpha(particle) as f32],
                    blend: BlendMode::Add,
                },
            );
        }
    }

    fn draw_statue(&self, renderer: &mut Renderer, camera: &Camera, statue: HubStatueOffsets) {
        // The JS planner places the aura at body depth - 0.25, with no intervening actor.
        self.draw_registered_blended(
            renderer,
            camera,
            STATUE_AURA,
            statue.aura,
            WHITE,
            BlendMode::Multiply,
        );
        self.draw_registered(renderer, camera, STATUE_BODY, statue.body, WHITE);
    }

    fn draw_southern(&self, renderer: &mut Renderer, camera: &Camera) {
        let camera = southern_camera(camera, renderer.logical_size());
        for &(index, origin) in &self.southern {
            let record = &self.records[index];
            renderer.draw_clamped(
                &self.atlas,
                Quad {
                    source: atlas_source(record),
                    destination: camera.project(
                        origin.x,
                        origin.y,
                        f64::from(record.width),
                        f64::from(record.height),
                    ),
                    tint: WHITE,
                    blend: BlendMode::Normal,
                },
            );
        }
    }

    /// Tiles the floor from the world origin, clipping the last column and row to the room.
    fn draw_floor(&self, renderer: &mut Renderer, camera: &Camera) {
        let tile = &self.records[FLOOR_TILE];
        let (tile_width, tile_height) = (f64::from(tile.width), f64::from(tile.height));
        let mut top = 0.0;
        while top < HUB_WORLD_SIZE.y {
            let height = tile_height.min(HUB_WORLD_SIZE.y - top);
            let mut left = 0.0;
            while left < HUB_WORLD_SIZE.x {
                let width = tile_width.min(HUB_WORLD_SIZE.x - left);
                renderer.draw_clamped(
                    &self.atlas,
                    Quad {
                        source: [tile.x as f32, tile.y as f32, width as f32, height as f32],
                        destination: camera.project(left, top, width, height),
                        tint: WHITE,
                        blend: BlendMode::Normal,
                    },
                );
                left += tile_width;
            }
            top += tile_height;
        }
    }

    fn draw_registered(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        index: usize,
        offset: Vec2,
        tint: [f32; 4],
    ) {
        self.draw_registered_blended(renderer, camera, index, offset, tint, BlendMode::Normal);
    }

    fn draw_registered_blended(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        index: usize,
        offset: Vec2,
        tint: [f32; 4],
        blend: BlendMode,
    ) {
        let record = &self.records[index];
        let [left, top] = registered_origin(record);
        renderer.draw_clamped(
            &self.atlas,
            Quad {
                source: atlas_source(record),
                destination: camera.project(
                    left + offset.x,
                    top + offset.y,
                    f64::from(record.width),
                    f64::from(record.height),
                ),
                tint,
                blend,
            },
        );
    }

    /// Draws a record at `scale` times its registered placement, the way the Website's
    /// `build_scaled_registered_world_layer` composes it.
    fn draw_scaled_additive(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        index: usize,
        scale: f64,
        tint: [f32; 4],
    ) {
        let record = &self.records[index];
        let [left, top] = registered_origin_scaled(record, scale);
        renderer.draw_clamped(
            &self.atlas,
            Quad {
                source: atlas_source(record),
                destination: camera.project(
                    left,
                    top,
                    (f64::from(record.width) * scale).round_ties_even(),
                    (f64::from(record.height) * scale).round_ties_even(),
                ),
                tint,
                blend: BlendMode::Add,
            },
        );
    }
}

/// `HubWorldScene::addSouthernArchitecture` and `hub-camera-presentation.ts`.
/// This bank uses cropped College records in a separate 1.25 camera scope.
fn southern_architecture() -> Vec<(usize, Vec2)> {
    let extent = Vec2::new(
        1.25 * HUB_WORLD_SIZE.x - 0.25 * HUB_LOGICAL_VIEW.x / HUB_CAMERA_SCALE / 2.0,
        1.25 * HUB_WORLD_SIZE.y - 0.25 * HUB_LOGICAL_VIEW.y / HUB_CAMERA_SCALE / 2.0,
    );
    let mut pieces = Vec::new();
    let mut x = 90.0;
    let mut slot = 0;
    let mut previous_was_tower = false;
    while x < extent.x {
        if slot == 4 || slot == 6 {
            pieces.push((44, Vec2::new(x, extent.y - 186.0)));
            x += 179.0;
            previous_was_tower = true;
        } else {
            let y = extent.y - 126.0 + if slot == 5 { 0.0 } else { 30.0 };
            pieces.push((4, Vec2::new(x, y)));
            if previous_was_tower {
                pieces.push((3, Vec2::new(x - 1.0, y)));
            }
            x += 209.0;
            previous_was_tower = false;
        }
        slot += 1;
    }
    pieces.push((7, Vec2::new(128.0 / HUB_CAMERA_SCALE, extent.y - 407.0)));
    pieces.push((43, Vec2::new(1843.0, extent.y - 415.0)));
    pieces
}

fn southern_camera(camera: &Camera, logical_view: [f32; 2]) -> Camera {
    Camera {
        origin: Vec2::new(
            1.25 * camera.origin.x + 0.25 * f64::from(logical_view[0]) / camera.scale / 2.0,
            1.25 * camera.origin.y + 0.25 * f64::from(logical_view[1]) / camera.scale / 2.0,
        ),
        scale: camera.scale,
    }
}

/// Top-left of a record's trimmed pixels inside its logical frame, rounded the way the
/// Website's atlas tooling rounds it (half to even).
pub fn registered_origin(record: &SpriteRecord) -> [f64; 2] {
    registered_origin_scaled(record, 1.0)
}

/// `registered_origin` for a record drawn at `scale` times its size: the tooling scales the
/// exact origin, then rounds.
pub fn registered_origin_scaled(record: &SpriteRecord, scale: f64) -> [f64; 2] {
    let left = (f64::from(record.logical_width) - f64::from(record.width)) / 2.0
        + f64::from(record.center_x);
    let top = (f64::from(record.logical_height) - f64::from(record.height)) / 2.0
        + f64::from(record.center_y);
    [(left * scale).round_ties_even(), (top * scale).round_ties_even()]
}

/// The record's atlas rectangle.
fn atlas_source(record: &SpriteRecord) -> [f32; 4] {
    [record.x as f32, record.y as f32, record.width as f32, record.height as f32]
}

/// Pixi quantises a sprite tint to eight bits per channel; the alpha stays a float.
fn tint(color: HubColor) -> [f32; 4] {
    let channel = |value: f64| ((value * 255.0).round() / 255.0) as f32;
    [channel(color.red), channel(color.green), channel(color.blue), color.alpha as f32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn southern_bank_keeps_the_central_gate_and_platforms() {
        let pieces = southern_architecture();
        // Authored gate: two towers flank a lowered battlement and its left seam.
        assert_eq!(
            &pieces[4..8],
            &[
                (44, Vec2::new(926.0, 1000.25)),
                (4, Vec2::new(1105.0, 1060.25)),
                (3, Vec2::new(1104.0, 1060.25)),
                (44, Vec2::new(1314.0, 1000.25)),
            ]
        );
        assert_eq!(pieces.last(), Some(&(43, Vec2::new(1843.0, 771.25))));
        let west = pieces[pieces.len() - 2];
        assert_eq!(west.0, 7);
        assert!((west.1.x - 106.666_666_666_666_67).abs() < 1e-10);
        assert_eq!(west.1.y, 779.25);
    }

    #[test]
    fn southern_camera_uses_the_dynamic_view_center() {
        let camera = Camera { origin: Vec2::new(100.0, 40.0), scale: 1.2 };
        let southern = southern_camera(&camera, [1600.0, 1000.0]);
        assert!((southern.origin.x - 291.666_666_666_666_7).abs() < 1e-10);
        assert!((southern.origin.y - 154.166_666_666_666_69).abs() < 1e-10);
        assert_eq!(southern.scale, 1.2);
    }
}
