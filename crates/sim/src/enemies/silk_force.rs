//! Native Silk force handoff into illuminated fade-line segments, from
//! `native-silk-force.ts` and `enemies/silk-force.ts`.
use super::silk::NativeSilkState;
use super::world::EnemyWorld;
use crate::float::{atan2, fround, hypot};
use crate::spells::targeting::direction_from_heading;
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeFadeLineState {
    pub start: Vec2,
    pub middle: Vec2,
    pub end: Vec2,
    pub velocity: Vec2,
    pub opacity: f64,
    pub opacity_loss_per_tick: f64,
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeFadeLineActor {
    pub id: usize,
    pub spawn_tick: u64,
    pub state: NativeFadeLineState,
}

#[must_use]
pub fn create_native_silk_fragments(
    silk: &NativeSilkState,
    force: Vec2,
    light_at: &dyn Fn(Vec2) -> f64,
    rng: &mut NativeRng,
) -> Vec<NativeFadeLineState> {
    let mut fragments = Vec::new();
    let lower = (silk.phase - 16.0).max(0.0);
    let step = fround(4.0 / silk.wave_scale) * 0.4000000059604645;
    let heading = atan2(force.x, -force.y) * 180.0 / std::f64::consts::PI;
    let mut cursor = silk.phase;
    while cursor > lower {
        let start = fragment_point(silk, cursor);
        let end = fragment_point(silk, fround(cursor - step));
        let middle = fragment_point(silk, fround(cursor - step * 0.5));
        let mean = Vec2::new(fround((start.x + end.x) * 0.5), fround((start.y + end.y) * 0.5));
        let light = light_at(mean);
        if light > 0.0 {
            let falloff = fround(
                1.0 - (hypot(mean.x - silk.position.x, mean.y - silk.position.y) / 300.0).min(1.0),
            );
            let angle = rng.draw_scaled_float(5.0, true);
            let direction = direction_from_heading(fround(heading + angle));
            let speed = rng.draw_scaled_float(0.09999990463256836, false);
            let opacity = rng.draw_scaled_float(0.55_f32 as f64, false);
            let loss = rng.draw_scaled_float(0.05_f32 as f64, false);
            let magnitude = fround(1.9500000476837158 + speed);
            fragments.push(NativeFadeLineState {
                start,
                middle,
                end,
                velocity: Vec2::new(
                    fround(fround(falloff * direction.x) * magnitude),
                    fround(fround(falloff * direction.y) * magnitude),
                ),
                opacity: fround(fround(fround(0.95) + opacity) * light * fround(0.35)),
                opacity_loss_per_tick: fround((fround(0.2) - loss) * fround(0.1)),
            });
        }
        cursor = fround(cursor - step);
    }
    fragments
}

fn fragment_point(silk: &NativeSilkState, phase: f64) -> Vec2 {
    let center = silk.center.evaluate(phase);
    let wave = silk.wave.evaluate(phase);
    Vec2::new(
        fround(center.x + fround(wave.x * silk.wave_scale) - silk.height),
        fround(center.y + fround(wave.y * silk.wave_scale) - silk.height),
    )
}
pub fn step_native_fade_line(mut source: NativeFadeLineState) -> Option<NativeFadeLineState> {
    source.opacity = fround(source.opacity - source.opacity_loss_per_tick);
    if source.opacity <= 0.0 {
        return None;
    }
    for point in [&mut source.start, &mut source.middle, &mut source.end] {
        *point =
            Vec2::new(fround(point.x + source.velocity.x), fround(point.y + source.velocity.y));
    }
    Some(source)
}

impl EnemyWorld {
    pub fn apply_silk_force(
        &mut self,
        id: usize,
        amount: f64,
        direction: Vec2,
        tick: u64,
        rng: &mut NativeRng,
        light_at: &dyn Fn(Vec2) -> f64,
    ) {
        let Some(index) = self.silks.iter().position(|silk| silk.id == id) else {
            return;
        };
        let accumulator = fround(self.silks[index].state.force_accumulator + amount);
        if accumulator <= 1.0 {
            self.silks[index].state.force_accumulator = accumulator;
            return;
        }
        let fragments =
            create_native_silk_fragments(&self.silks[index].state, direction, light_at, rng);
        self.silks.remove(index);
        for state in fragments {
            self.silk_fragments.push(NativeFadeLineActor {
                id: self.next_death_effect_id,
                spawn_tick: tick,
                state,
            });
            self.next_death_effect_id += 1;
        }
    }
    pub fn step_silk_fragments(&mut self) {
        self.silk_fragments.retain_mut(|actor| {
            if let Some(state) = step_native_fade_line(actor.state.clone()) {
                actor.state = state;
                true
            } else {
                false
            }
        });
    }
}
