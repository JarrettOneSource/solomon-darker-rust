//! Native Golem assembly's four twenty-four-piece bursts from `native-golem-debris.ts`.

use crate::float::{self, fround};
use crate::spells::earth_debris::{
    NativeWeldBoulderDebrisParticleState, NativeWeldMeteorDebrisSeed,
    create_native_weld_boulder_debris_particle,
};
use crate::{NativeRng, Vec2};

#[must_use]
pub fn create_native_golem_assembly_debris(
    rng: &mut NativeRng,
    scale: f64,
    enhanced: bool,
) -> Vec<NativeWeldBoulderDebrisParticleState> {
    assert!(scale.is_finite() && scale > 0.0, "Golem scale must be positive");
    let mut particles = Vec::with_capacity(24);
    for index in 0..24 {
        let angle = fround(index as f64 * 15.0 + rng.draw_scaled_float(5.0, true))
            * std::f64::consts::PI
            / 180.0;
        let bounce_velocity = fround(-(rng.draw_scaled_float(3.0, false) + 2.0));
        rng.draw_scaled_float(20.0, false); // The assembly recipe overwrites constructor height.
        let rotation_degrees = rng.draw_scaled_float(360.0, false);
        let rotation_step_degrees = fround(1.0 + rng.draw_scaled_float(10.0, false));
        let record = 2008 + rng.draw_integer(3) as usize;
        let distance = rng.draw_scaled_float(25.0, false);
        let mut position =
            Vec2::new(fround(float::sin(angle) * distance), fround(-float::cos(angle) * distance));
        // Retail's second heading conversion reads the radius register.
        let radians = distance * std::f64::consts::PI / 180.0;
        let direction =
            Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians) * (0.8_f32 as f64)));
        let vertical_scale =
            fround(fround(scale.min(1.0) * rng.draw_scaled_float(1.5, false) + 0.75) * 0.5);
        let vertical_velocity = fround(bounce_velocity * vertical_scale);
        let height = fround(-rng.draw_scaled_float(fround(scale.min(1.0) * 50.0), false));
        let lead = rng.draw_scaled_float(30.0, false);
        position = Vec2::new(
            fround(position.x + fround(direction.x * lead)),
            fround(position.y + fround(direction.y * lead)),
        );
        let mut size = 0.45_f32 as f64;
        let probe = fround(fround(0.5 + rng.draw_scaled_float(0.75, false)) * scale);
        if probe >= size {
            size = fround(fround(0.5 + rng.draw_scaled_float(0.75, false)) * scale);
        }
        size = size.min(0.75_f32 as f64);
        size = fround(
            size * fround(0.30000001192092896 + rng.draw_scaled_float(0.34999996423721313, false)),
        );
        let speed = fround(1.0 + rng.draw_scaled_float(fround(scale * 0.5), false));
        let velocity = Vec2::new(fround(direction.x * speed), fround(direction.y * speed));
        let mut particle = create_native_weld_boulder_debris_particle(
            NativeWeldMeteorDebrisSeed {
                alpha: 2.0,
                color_green: 1.0,
                height,
                index,
                position,
                record,
                rotation_degrees,
                rotation_step_degrees,
                scale: size,
                velocity,
                vertical_velocity,
            },
            enhanced,
        );
        particle.bounce_velocity = bounce_velocity;
        particles.push(particle);
    }
    particles
}
