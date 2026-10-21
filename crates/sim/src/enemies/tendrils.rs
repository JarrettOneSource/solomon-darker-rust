//! Heartmonger's retained foot chains from `renderer/native-heartmonger-tendrils.ts`.

use super::actor_presentation::{EnemyArtAtlas, EnemySpriteLayer};
use crate::float;
use crate::float::{atan2, fround};
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, PartialEq)]
pub struct HeartmongerTendril {
    pub lengths: Vec<f64>,
    pub points: Vec<Vec2>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeartmongerTendrils {
    pub chains: Vec<HeartmongerTendril>,
    pub leg_pose: i64,
}

impl HeartmongerTendrils {
    /// The second chain continues the first constructor's walk; it does not reset its root.
    #[must_use]
    pub fn new(position: Vec2, heading_deg: f64, rng: &mut NativeRng) -> Self {
        let radians = fround(fround(std::f64::consts::PI) * fround(heading_deg + 180.0) / 180.0);
        let direction = Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)));
        let mut current = position;
        let mut chains = Vec::new();
        for count in [7, 4] {
            let mut points = Vec::new();
            for _ in 0..count {
                points.push(current);
                let distance = fround(25.0 + rng.draw_scaled_float(4.0, true));
                current = Vec2::new(
                    fround(current.x + direction.x * distance),
                    fround(current.y + direction.y * distance),
                );
            }
            let mut lengths = Vec::new();
            for pair in points.windows(2) {
                lengths.push(fround(crate::float::hypot(
                    pair[1].x - pair[0].x,
                    pair[1].y - pair[0].y,
                )));
            }
            chains.push(HeartmongerTendril { lengths, points });
        }
        Self { chains, leg_pose: -1 }
    }

    /// Fixed native segment lengths advance only when the integer leg pose changes.
    pub fn move_to(&mut self, leg_phase: f64, position: Vec2, anchors: &[Vec2]) {
        let pose = leg_phase.trunc() as i64;
        if pose == self.leg_pose {
            return;
        }
        for (index, chain) in self.chains.iter_mut().enumerate() {
            let anchor = anchors.get(index).expect("Heartmonger legs require both tendril anchors");
            let mut points =
                vec![Vec2::new(fround(position.x + anchor.x), fround(position.y + anchor.y))];
            for point in 1..chain.points.len() {
                let previous = points[point - 1];
                let original = chain.points[point];
                let dx = fround(original.x - previous.x);
                let dy = fround(original.y - previous.y);
                let distance = fround(fround(dx * dx + dy * dy).sqrt());
                let scale =
                    if distance > 0.0 { fround(chain.lengths[point - 1] / distance) } else { 0.0 };
                points.push(Vec2::new(
                    fround(previous.x + fround(dx * scale)),
                    fround(previous.y + fround(dy * scale)),
                ));
            }
            chain.points = points;
        }
        self.leg_pose = pose;
    }

    #[must_use]
    pub fn layers(&self, root: Vec2, shadow_direction: Vec2) -> Vec<EnemySpriteLayer> {
        let mut layers = Vec::new();
        for shadow in [true, false] {
            for chain in &self.chains {
                for points in chain.points.windows(2) {
                    let start = points[0];
                    let end = points[1];
                    layers.push(EnemySpriteLayer {
                        offset: Vec2::new(
                            start.x - root.x + if shadow { 3.0 * shadow_direction.x } else { 0.0 },
                            start.y - root.y + if shadow { 3.0 * shadow_direction.y } else { 0.0 },
                        ),
                        rotation_radians: atan2(end.x - start.x, start.y - end.y),
                        tint: if shadow { 0 } else { 0xffffff },
                        ..EnemySpriteLayer::new(
                            EnemyArtAtlas::BadGuys,
                            19,
                            if shadow {
                                "heartmonger-tendril-shadow"
                            } else {
                                "heartmonger-tendril"
                            },
                        )
                    });
                }
            }
        }
        layers
    }
}
