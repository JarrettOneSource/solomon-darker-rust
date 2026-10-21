//! The ordinary Bouncer births from `native-secondary-abilities.ts`:
//! `spawnStoneskinBurst` and `spawnStoneskinChip` share the secondary RNG.

use super::{NativeSecondaryActorKind, NativeSecondaryActorState, NativeSecondarySimulationState};
use crate::Vec2;
use crate::float::{cos, fround, sin};

impl NativeSecondarySimulationState {
    pub(super) fn spawn_stoneskin_burst(
        &mut self,
        owner_id: &str,
        origin: Vec2,
        world_key: &str,
        enhanced_effects: bool,
    ) {
        for heading in (0..360).step_by(20) {
            self.spawn_stoneskin_chip(
                owner_id,
                origin,
                world_key,
                enhanced_effects,
                Some(f64::from(heading)),
            );
        }
    }

    pub(super) fn spawn_stoneskin_chip(
        &mut self,
        owner_id: &str,
        origin: Vec2,
        world_key: &str,
        enhanced_effects: bool,
        burst_heading: Option<f64>,
    ) {
        let heading = match burst_heading {
            Some(heading) => heading,
            None => self.rng.draw_scaled_float(360.0, false),
        };
        let bounce = fround(-(self.rng.draw_scaled_float(3.0, false) + 2.0));
        let height = fround(-self.rng.draw_scaled_float(20.0, false));
        let rotation = self.rng.draw_scaled_float(360.0, false) * std::f64::consts::PI / 180.0;
        let spin =
            fround(1.0 + self.rng.draw_scaled_float(10.0, false)) * std::f64::consts::PI / 180.0;
        let angle =
            fround(heading + self.rng.draw_scaled_float(10.0, true)) * std::f64::consts::PI / 180.0;
        let speed = fround(0.5 + self.rng.draw_scaled_float(0.5, false));
        let velocity =
            Vec2::new(fround(fround(sin(angle) * 1.5) * speed), fround(-cos(angle) * speed));
        let lead = fround(15.0 + self.rng.draw_scaled_float(10.0, false));
        let mut position = Vec2::new(
            fround(origin.x + fround(velocity.x * lead)),
            fround(origin.y + fround(velocity.y * lead)),
        );
        // This factory's last X offset is one multiply and sum, unlike Harden's
        // two left-associated additions. Keep the original float-store boundary.
        position.x = fround(position.x + velocity.x * 2.0);
        if burst_heading.is_none() {
            self.rng.draw_scaled_float(0.2, true); // Separate physical-contact audio owner.
        }
        let mut chip = NativeSecondaryActorState::new(
            0,
            NativeSecondaryActorKind::StoneskinChip,
            owner_id.into(),
            world_key.into(),
            Some(46),
        );
        chip.enhanced = enhanced_effects;
        chip.alpha = if burst_heading.is_some() && enhanced_effects { 10.0 } else { 2.0 };
        chip.variant = i32::from(burst_heading.is_some());
        chip.lifetime_ticks = 1000;
        chip.phase = height;
        chip.endpoint = Vec2::new(bounce, bounce);
        chip.position = position;
        chip.velocity = velocity;
        chip.rotation_radians = rotation;
        chip.slow_factor = spin;
        chip.frame = 77.0;
        self.spawn_actor(chip);
    }
}
