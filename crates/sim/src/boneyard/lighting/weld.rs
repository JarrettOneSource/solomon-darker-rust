//! Light sources for Weld projectiles, rocks and meteors. Callers pass each actor's own
//! fields; no actor clocks live here.

use crate::Vec2;
use crate::float::fround;
use crate::profile::WeldBuildId;

use super::{BoneyardLightSource, missile_light_source, presentation_random};

#[must_use]
pub fn weld_projectile_light_source(
    build_id: WeldBuildId,
    id: usize,
    position: Vec2,
    frame: u64,
    multiple_shadows: bool,
) -> BoneyardLightSource {
    if build_id != WeldBuildId::CrawlingShock {
        return missile_light_source(id as u32, position, frame, multiple_shadows);
    }
    BoneyardLightSource::new(
        position,
        0.4_f32 as f64,
        fround(0.5 + presentation_random(frame, id as u32 ^ 0x5e7800, 0.5)),
        false,
    )
}

#[must_use]
pub fn weld_rock_light_source(
    origin: Vec2,
    scale: f64,
    multiple_shadows: bool,
) -> BoneyardLightSource {
    BoneyardLightSource::new(origin, fround(scale * 0.75).max(0.5), 0.5, multiple_shadows)
}

#[must_use]
pub fn weld_meteor_light_source(
    position: Vec2,
    fall_height: f64,
    impact: bool,
    impact_ticks_remaining: u64,
    impact_radius_scalar: f64,
) -> Option<BoneyardLightSource> {
    if fall_height > 1.0 {
        return None;
    }
    let visibility = if impact { impact_ticks_remaining.min(50) as f64 / 50.0 } else { 1.0 };
    Some(BoneyardLightSource::new(
        position,
        fround(impact_radius_scalar * (0.6_f32 as f64)),
        fround(visibility * fround(1.0 - fall_height)).min(1.0),
        false,
    ))
}
