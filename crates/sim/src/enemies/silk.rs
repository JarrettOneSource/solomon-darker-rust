//! Silk's two QuickSplines and fixed-tick flight from `core-kernels/native-silk.ts`.
use crate::float;
use crate::float::{fround, hypot};
use crate::spline::NaturalSpline;
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSilkState {
    pub force_accumulator: f64,
    pub age_ticks: u64,
    pub phase: f64,
    pub ended: bool,
    pub position: Vec2,
    pub center: NaturalSpline,
    pub wave: NaturalSpline,
    pub height: f64,
    pub alpha: f64,
    pub wave_scale: f64,
    pub drift: Vec2,
    pub drift_velocity: Vec2,
    pub cocoon_health: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeSilkCreation {
    pub state: NativeSilkState,
    pub sound_index: usize,
    pub sound_pitch: f64,
    pub sound_position: Vec2,
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemySilkActor {
    pub native_registration_order: usize,
    pub native_cell_binding_order: usize,
    pub id: usize,
    pub owner_actor_id: usize,
    pub spawn_tick: u64,
    pub state: NativeSilkState,
    pub painter_registration: crate::painter::PainterRegistration,
}

#[must_use]
pub fn create_native_silk(
    position: Vec2,
    target_position: Vec2,
    target_velocity: Vec2,
    cocoon_health: f64,
    rng: &mut NativeRng,
) -> NativeSilkCreation {
    let height = rng.draw_scaled_float(10.0, false);
    rng.draw_scaled_float(2.0, false);
    let lead = rng.draw_float_range(2.7_f32 as f64, 2.9_f32 as f64);
    let distance_to_target = hypot(target_position.x - position.x, target_position.y - position.y);
    let aimed = Vec2::new(
        target_position.x + distance_to_target / lead * target_velocity.x,
        target_position.y + distance_to_target / lead * target_velocity.y,
    );
    let dx = aimed.x - position.x;
    let dy = aimed.y - position.y;
    let distance = hypot(dx, dy);
    let direction =
        if distance == 0.0 { Vec2::ZERO } else { Vec2::new(dx / distance, dy / distance) };
    let extra = rng.draw_scaled_float(300.0, false);
    let length = distance + 100.0 + extra;
    let waves = rng.draw_scaled_float(12.0, false);
    let mut center = Vec::new();
    let mut wave = Vec::new();
    let mut t = 0.0;
    while t < 1.0 {
        center.push(Vec2::new(
            fround(position.x + length * direction.x * t),
            fround(position.y + length * direction.y * t),
        ));
        let amplitude = rng.draw_scaled_float(5.0, false);
        let offset = float::sin(t * (20.0 + waves) * std::f64::consts::PI) * (5.0 + amplitude);
        wave.push(Vec2::new(fround(direction.y * offset), fround(-direction.x * offset)));
        t = fround(t + fround(0.05));
    }
    let drift = rng.draw_scaled_float(2.0, false);
    let pitch = rng.draw_scaled_float(0.15000003576278687, false);
    let sound = rng.draw_integer(3) as usize;
    NativeSilkCreation {
        state: NativeSilkState {
            force_accumulator: 0.0,
            age_ticks: 0,
            phase: 0.0,
            ended: false,
            position,
            center: NaturalSpline::new(&center),
            wave: NaturalSpline::new(&wave),
            height: fround(18.0 + height),
            alpha: 1.0,
            wave_scale: 0.5,
            drift: Vec2::ZERO,
            drift_velocity: Vec2::new(
                fround((drift - 1.0) * direction.y / 6.0),
                fround(-(drift - 1.0) * direction.x / 6.0),
            ),
            cocoon_health,
        },
        sound_index: sound,
        sound_pitch: fround(fround(0.95) + pitch),
        sound_position: Vec2::new(
            position.x + direction.x * length * 0.5,
            position.y + direction.y * length * 0.5,
        ),
    }
}

pub fn step_native_silk(mut state: NativeSilkState) -> Option<NativeSilkState> {
    state.age_ticks += 1;
    if !state.ended && state.age_ticks.is_multiple_of(4) {
        return Some(state);
    }
    state.height = fround(state.height * fround(if state.ended { 0.98 } else { 0.995 }));
    if !state.ended {
        state.phase = fround(state.phase + fround(0.15));
        state.ended = state.phase > state.wave.extent;
        let center = state.center.evaluate(state.phase);
        let wave = state.wave.evaluate(state.phase);
        state.position = Vec2::new(fround(center.x + wave.x), fround(center.y + wave.y));
    } else {
        if state.height < 0.25 {
            state.alpha = fround(state.alpha - fround(0.01));
        }
        if state.alpha <= 0.0 {
            return None;
        }
        let damping = fround(if state.height <= 1.0 { 0.98 } else { 1.01 });
        state.drift_velocity = Vec2::new(
            fround(state.drift_velocity.x * damping),
            fround(state.drift_velocity.y * damping),
        );
    }
    state.drift = Vec2::new(
        fround(state.drift.x + state.drift_velocity.x),
        fround(state.drift.y + state.drift_velocity.y),
    );
    if state.phase > state.wave.extent / 3.0 {
        state.wave_scale = fround(state.wave_scale * fround(0.995));
    }
    Some(state)
}
