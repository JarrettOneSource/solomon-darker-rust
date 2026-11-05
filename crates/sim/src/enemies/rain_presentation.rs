//! Rain's private paint-time manager from `renderer/native-rain-cloud.ts` and
//! `core-kernels/native-faculty-spells.ts::createNativeRainCloudBillow`.

use super::actor_presentation::{EnemyArtAtlas, EnemySpriteLayer};
use super::boss_spell::{BossSpellKind, NativeBossSpell};
use crate::float;
use crate::float::fround;
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, PartialEq)]
pub struct RainCloudBillow {
    pub entry: usize,
    pub position: Vec2,
    pub phase_step_deg: f64,
    pub rotation_deg: f64,
    pub scale: f64,
    pub tint: u32,
    pub phase_deg: f64,
}

/// Cosmetic RNG and children belong to painting, independently of the world actor clock.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeRainCloud {
    pub billows: Vec<RainCloudBillow>,
    pub last_birth_tick: u64,
    pub rng: NativeRng,
}

impl NativeRainCloud {
    #[must_use]
    pub fn new(spell: &NativeBossSpell, presentation_tick: u64) -> Self {
        assert!(
            matches!(spell.kind, BossSpellKind::RainOfBones { .. }),
            "Rain Cloud requires Rain of Bones"
        );
        Self {
            billows: Vec::new(),
            last_birth_tick: if spell.owner.age_ticks <= 1 {
                spell.owner.spawn_tick
            } else {
                presentation_tick
            },
            rng: NativeRng::new(spell.owner.id as i64),
        }
    }

    #[must_use]
    pub fn update(
        &mut self,
        spell: &NativeBossSpell,
        presentation_tick: u64,
    ) -> Vec<EnemySpriteLayer> {
        let BossSpellKind::RainOfBones { alpha, .. } = spell.kind else {
            panic!("Rain Cloud requires Rain of Bones")
        };
        if alpha <= 0.0 {
            return Vec::new();
        }
        let through = presentation_tick.min(spell.owner.spawn_tick + 1199);
        let first = (self.last_birth_tick + 1).max(spell.owner.spawn_tick + 20);
        for _ in first..=through {
            for _ in 0..2 {
                self.billows.push(create_billow(spell.owner.position, &mut self.rng));
            }
        }
        self.last_birth_tick = presentation_tick;
        self.billows.retain_mut(|billow| {
            billow.phase_deg = fround(billow.phase_deg + billow.phase_step_deg);
            billow.phase_deg < 180.0
        });
        let mut layers = Vec::new();
        for (index, billow) in self.billows.iter().enumerate() {
            layers.push(EnemySpriteLayer {
                alpha: fround(float::sin(billow.phase_deg * std::f64::consts::PI / 180.0)),
                offset: Vec2::new(
                    billow.position.x - spell.owner.position.x,
                    billow.position.y - spell.owner.position.y - 175.0,
                ),
                rotation_radians: billow.rotation_deg * std::f64::consts::PI / 180.0,
                scale: billow.scale,
                tint: billow.tint,
                ..EnemySpriteLayer::new(
                    EnemyArtAtlas::BadGuys,
                    billow.entry,
                    format!("acid-pain-cloud-billow-{index}"),
                )
            });
        }
        layers
    }
}

fn create_billow(position: Vec2, rng: &mut NativeRng) -> RainCloudBillow {
    let entry = 10 + rng.draw_integer(2) as usize;
    let radius = rng.draw_scaled_float(200.0, false);
    let angle = rng.draw_scaled_float(360.0, false) * std::f64::consts::PI / 180.0;
    let rise = rng.draw_scaled_float(3.0, false);
    let rotation = rng.draw_scaled_float(360.0, false);
    let scale = rng.draw_scaled_float(0.8500000238418579, false);
    rng.draw_scaled_float(f64::from(0.02_f32), false);
    let red = rng.draw_scaled_float(f64::from(0.15_f32), false);
    let direction = Vec2::new(fround(float::sin(angle)), fround(-float::cos(angle)));
    RainCloudBillow {
        entry,
        position: Vec2::new(
            fround(position.x + fround(radius * direction.x)),
            fround(position.y + fround(fround(radius * direction.y) * f64::from(0.8_f32))),
        ),
        phase_step_deg: fround(2.0 + rise),
        rotation_deg: rotation,
        scale: fround(f64::from(0.9_f32) + scale),
        tint: ((red * 255.0).round() as u32) << 16,
        phase_deg: 0.0,
    }
}
