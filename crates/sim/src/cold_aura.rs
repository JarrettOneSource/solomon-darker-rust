//! Cold Aura's cached radius scale, alpha decay, lifetime and fading visual, with each step
//! stored at single precision. Radius is the authored feet/7 skill cache; target queries
//! convert it separately.
use crate::float::fround;

pub const COLD_AURA_PERIOD_TICKS: u64 = 6;
const INITIAL_ALPHA: f64 = 0.5_f32 as f64;
const SCALE_PER_TICK: f64 = 1.015_f32 as f64;
const RED_LOSS_PER_TICK: f64 = 0.02_f32 as f64;

#[must_use]
pub fn cold_aura_radius_scale(feet: f64) -> f64 {
    fround(feet / 7.0)
}
#[must_use]
pub fn cold_aura_query_radius(radius_scale: f64) -> f64 {
    fround(radius_scale * 120.0)
}
#[must_use]
pub fn cold_aura_alpha_decay(radius_scale: f64) -> f64 {
    assert!(
        radius_scale.is_finite() && radius_scale > 0.0,
        "Cold Aura radius scale is positive and finite"
    );
    let base_loss = fround(fround(fround(0.1) * fround(0.1)) * 0.5);
    let decay = fround(fround(base_loss / radius_scale) * 1.25);
    validate_alpha_decay(decay);
    decay
}
#[must_use]
pub fn cold_aura_lifetime_ticks(alpha_decay: f64) -> u64 {
    validate_alpha_decay(alpha_decay);
    let mut alpha = INITIAL_ALPHA;
    let mut ticks = 0;
    while alpha > 0.0 {
        alpha = fround(alpha - alpha_decay);
        ticks += 1;
    }
    ticks
}
/// One emitted Cold Aura visual's age, alpha decay and rotation parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColdAuraVisualInput {
    pub age_ticks: f64,
    pub alpha_decay: f64,
    pub initial_rotation_degrees: f64,
    pub rotation_step_degrees: f64,
}
/// A Cold Aura visual at its current age: alpha, red channel, rotation and scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColdAuraVisualState {
    pub alpha: f64,
    pub red: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
}
#[must_use]
pub fn cold_aura_visual_state(input: ColdAuraVisualInput) -> ColdAuraVisualState {
    validate_alpha_decay(input.alpha_decay);
    let mut alpha = INITIAL_ALPHA;
    let mut red = fround(1.0);
    let mut rotation_degrees = fround(input.initial_rotation_degrees);
    let mut scale = fround(1.0);
    let age = input.age_ticks.max(0.0).trunc() as u64;
    for _ in 0..age {
        if alpha <= 0.0 {
            break;
        }
        alpha = fround(alpha - input.alpha_decay);
        scale = fround(scale * SCALE_PER_TICK);
        rotation_degrees = fround(rotation_degrees + input.rotation_step_degrees);
        red = fround(red - RED_LOSS_PER_TICK).max(0.0);
    }
    ColdAuraVisualState { alpha: alpha.max(0.0), red, rotation_degrees, scale }
}
fn validate_alpha_decay(decay: f64) {
    assert!(
        decay.is_finite() && decay > 0.0 && fround(INITIAL_ALPHA - decay) != INITIAL_ALPHA,
        "Cold Aura alpha decay makes finite native progress"
    );
}
