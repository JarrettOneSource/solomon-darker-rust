//! Live scenery presentation from `boneyard-tree-occlusion.ts`, `native-fence-geometry.ts`
//! and `region-painter-order.ts`. Art is retained data; these rules remain live Rust state.

use crate::Vec2;
use crate::float::fround;
use crate::hub::painter::{PainterLayer, hub_painter_order};

use super::BoneyardBounds;
use super::lighting::stable_string_hash;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneyardWorldTransform {
    pub position: Vec2,
    pub scale: f64,
}

/// `nativeEnemyWorldFeedbackTransform`: zoom around the local wizard's screen
/// position, preserving its anchor while the surrounding world pulses.
#[must_use]
pub fn native_enemy_world_feedback_transform(
    camera_center: Vec2,
    zoom: f64,
    viewport: Vec2,
    player_position: Vec2,
    magnitude: f64,
) -> BoneyardWorldTransform {
    let base = Vec2::new(
        viewport.x / 2.0 - camera_center.x * zoom,
        viewport.y / 2.0 - camera_center.y * zoom,
    );
    let anchor = Vec2::new(base.x + player_position.x * zoom, base.y + player_position.y * zoom);
    let scale = zoom * (1.0 + magnitude);
    BoneyardWorldTransform {
        position: Vec2::new(
            anchor.x - player_position.x * scale,
            anchor.y - player_position.y * scale,
        ),
        scale,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeArenaDisplacementCover {
    pub position: Vec2,
    pub rectangles: [BoneyardBounds; 2],
}

/// Stock screen strips cover the exposed edge of the displaced light field.
/// Signed rectangle dimensions preserve the source's asymmetric edge behavior.
#[must_use]
pub fn native_arena_displacement_cover(
    displacement: Vec2,
    viewport: Vec2,
    complex_lighting: bool,
) -> Option<NativeArenaDisplacementCover> {
    let x = fround(displacement.x);
    let y = fround(displacement.y);
    let magnitude_squared = fround(x * x + y * y);
    if !complex_lighting || !magnitude_squared.is_finite() || magnitude_squared < 1.0 {
        return None;
    }
    Some(NativeArenaDisplacementCover {
        position: Vec2::new(x, y),
        rectangles: [
            BoneyardBounds::new(fround(-x), fround(-y), fround(viewport.x + x + x), y),
            BoneyardBounds::new(fround(-x), fround(-y), x, fround(viewport.y + y + y)),
        ],
    })
}

#[derive(Clone, Debug)]
pub struct TreeOcclusion {
    pub position: Vec2,
    pub bounds: [f64; 4],
    pub polygon: Vec<Vec2>,
    pub countdown: u32,
    pub current_alpha: f64,
    pub target_alpha: f64,
}

impl TreeOcclusion {
    #[must_use]
    pub fn new(id: &str, position: Vec2, bounds: [f64; 4], polygon: Vec<Vec2>) -> Self {
        assert!(polygon.len() >= 3, "a native tree canopy needs a polygon");
        Self {
            position,
            bounds,
            polygon,
            countdown: stable_string_hash(id) % 25,
            current_alpha: 1.0,
            target_alpha: 1.0,
        }
    }

    /// Approach the old target before scanning. None represents an unobserved
    /// or off-camera scan and preserves the previous target, as in current Source.
    pub fn step(&mut self, actor_inside: Option<bool>) {
        let step = 0.015_f32 as f64;
        if self.current_alpha < self.target_alpha {
            self.current_alpha = fround(self.current_alpha + step).min(self.target_alpha);
        } else if self.current_alpha > self.target_alpha {
            self.current_alpha = fround(self.current_alpha - step).max(self.target_alpha);
        }
        if self.countdown <= 1 {
            self.countdown = 25;
            if let Some(inside) = actor_inside {
                self.target_alpha = if inside { 0.4_f32 as f64 } else { 1.0 };
            }
        } else {
            self.countdown -= 1;
        }
    }

    pub fn advance_unobserved(&mut self, ticks: u64) {
        if ticks == 0 {
            return;
        }
        let countdown = self.countdown;
        for _ in 0..ticks {
            if self.current_alpha == self.target_alpha {
                break;
            }
            self.step(None);
        }
        let first_scan = u64::from(countdown.max(1));
        self.countdown = if ticks < first_scan {
            countdown - ticks as u32
        } else {
            25 - ((ticks - first_scan) % 25) as u32
        };
    }

    #[must_use]
    pub fn overlaps_camera(&self, camera: BoneyardBounds) -> bool {
        let [left, top, width, height] = self.bounds;
        let x = fround(left + fround(self.position.x));
        let y = fround(top + fround(self.position.y));
        x < fround(fround(camera.x) + fround(camera.width))
            && fround(x + width) > fround(camera.x)
            && y < fround(fround(camera.y) + fround(camera.height))
            && fround(y + height) > fround(camera.y)
    }

    #[must_use]
    pub fn contains(&self, player: Vec2) -> bool {
        let point = Vec2::new(
            fround(fround(player.x) - fround(self.position.x)),
            fround(fround(player.y) - fround(self.position.y)),
        );
        let [left, top, width, height] = self.bounds;
        if point.x < left
            || point.x >= fround(left + width)
            || point.y < top
            || point.y >= fround(top + height)
        {
            return false;
        }
        let mut inside = false;
        let mut previous = self.polygon.len() - 1;
        for index in 0..self.polygon.len() {
            let current = self.polygon[index];
            let last = self.polygon[previous];
            if (current.y > point.y) != (last.y > point.y)
                && point.x
                    < fround(
                        current.x
                            + fround(
                                fround(fround(point.y - current.y) * fround(last.x - current.x))
                                    / fround(last.y - current.y),
                            ),
                    )
            {
                inside = !inside;
            }
            previous = index;
        }
        inside
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateArt {
    /// Native order: upper hinge, upper tip, lower hinge, lower tip.
    pub vertices: [Vec2; 4],
    pub hinge_sprite: Vec2,
    pub lines: [[Vec2; 2]; 2],
    pub painter_y: f64,
}

#[must_use]
pub fn native_gate_art(hinge: Vec2, tip: Vec2) -> GateArt {
    let upper_hinge = Vec2::new(hinge.x, hinge.y - 87.0);
    let upper_tip = Vec2::new(tip.x, tip.y - 87.0);
    let midpoint = Vec2::new((hinge.x + tip.x) / 2.0, (hinge.y + tip.y) / 2.0);
    GateArt {
        vertices: [upper_hinge, upper_tip, hinge, tip]
            .map(|point| Vec2::new(fround(point.x), fround(point.y))),
        hinge_sprite: Vec2::new(
            (upper_hinge.x + upper_tip.x) / 2.0,
            (upper_hinge.y + upper_tip.y) / 2.0 + 7.0,
        ),
        lines: [
            [Vec2::new(upper_tip.x, upper_tip.y + 32.0), tip],
            [Vec2::new(midpoint.x, midpoint.y - 87.0), midpoint],
        ],
        painter_y: if tip.y < midpoint.y { midpoint.y } else { tip.y },
    }
}

/// A tree canopy or building roof enqueued while its owner's row is being painted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneyardPainterProxy {
    pub draw_id: usize,
    pub world_y: f64,
    pub sort_bias: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneyardPainterEntry {
    pub draw_id: usize,
    pub layer: PainterLayer,
    pub proxy: Option<BoneyardPainterProxy>,
}

/// Source manager lanes enter the row buckets first. A proxy joins its future row at the
/// tail when its parent paints, rather than sorting as an independently registered actor.
#[must_use]
pub fn boneyard_painter_order(entries: &[BoneyardPainterEntry], reference_y: f64) -> Vec<usize> {
    let mut layers = Vec::with_capacity(entries.len() * 2);
    let mut draw_ids = Vec::with_capacity(entries.len() * 2);
    for entry in entries {
        layers.push(entry.layer);
        draw_ids.push(entry.draw_id);
        if let Some(proxy) = entry.proxy {
            layers.push(PainterLayer {
                visible: true,
                is_insertion: true,
                manager_lane: entry.layer.manager_lane,
                registration_ordinal: entry.layer.registration_ordinal,
                world_y: proxy.world_y,
                sort_bias: proxy.sort_bias,
            });
            draw_ids.push(proxy.draw_id);
        }
    }
    // Trees, roofs and split spell bands all use the same native deferred row queue.
    hub_painter_order(&layers, reference_y).into_iter().map(|index| draw_ids[index]).collect()
}
