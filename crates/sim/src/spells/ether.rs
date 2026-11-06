//! Ether Blast: its charge, the health-scaled pulse and the stock particle program.
//! The live actor saves its starting RNG, so rendering can reconstruct the particles
//! without consuming world randomness.

pub use super::casting::{ETHER_BLAST_WEAPON_PULSE, PLAYER_WEAPON_PULSE_DECAY};
use super::targeting::direction_from_heading;
use crate::float::fround;
use crate::{Rng, Vec2};

pub const ETHER_BLAST_CHARGE_PER_TICK: f64 = 0.007_f32 as f64;
pub const ETHER_BLAST_CONTACT_RADIUS: f64 = 175.0;
pub const ETHER_BLAST_PARTICLE_LIFETIME_TICKS: u64 = 100;
pub const ETHER_BLAST_PRESENTATION_RNG_WORDS: usize = 720;
pub const ETHER_BLAST_SCREEN_FLASH_DECAY: f64 = 0.025_f32 as f64;

/// One tick of Ether Blast charging: the new charge and whether it crossed a whole charge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EtherBlastChargeStep {
    pub charge: f64,
    pub crossed_integer: bool,
}

/// One Ether Blast particle as constructed: offset, velocity, damping, rotation, scale,
/// color, alpha loss and sprite record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EtherBlastParticle {
    pub alpha_loss: f64,
    pub blue: f64,
    pub damping: f64,
    pub green: f64,
    pub offset: Vec2,
    pub red: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
    pub sprite_record: usize,
    pub velocity: Vec2,
}

/// An Ether Blast particle's position and alpha at a given age.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EtherBlastParticleFrame {
    pub particle: EtherBlastParticle,
    pub alpha: f64,
    pub position: Vec2,
}

#[must_use]
pub fn advance_ether_blast_charge(
    source: f64,
    capacity: u32,
    full_mana_available: bool,
    plane_active: bool,
) -> EtherBlastChargeStep {
    assert!(source.is_finite() && source >= 0.0, "Ether Blast charge must be nonnegative");
    if plane_active || !full_mana_available {
        return EtherBlastChargeStep { charge: 0.0, crossed_integer: false };
    }
    if capacity == 0 {
        return EtherBlastChargeStep { charge: fround(source), crossed_integer: false };
    }
    let charge = fround(fround(source) + ETHER_BLAST_CHARGE_PER_TICK).min(f64::from(capacity));
    EtherBlastChargeStep { charge, crossed_integer: charge.floor() > source.floor() }
}

#[must_use]
pub fn ether_blast_release_charges(charge: f64) -> u32 {
    assert!(charge.is_finite() && charge >= 0.0, "Ether Blast charge must be nonnegative");
    fround(charge).round() as u32
}

#[must_use]
pub fn ether_blast_pulse_origin(player_position: Vec2, direction: Vec2) -> Vec2 {
    Vec2::new(
        fround(player_position.x + direction.x * 100.0),
        fround(player_position.y + direction.y * 100.0),
    )
}

#[must_use]
pub fn ether_blast_damage(charges: u32, current_health: f64) -> f64 {
    assert!(charges > 0, "Ether Blast damage requires at least one charge");
    assert!(
        current_health.is_finite() && current_health >= 0.0,
        "target health must be nonnegative"
    );
    let factor = fround(fround(f64::from(charges)) * 0.15_f32 as f64).min(0.95_f32 as f64);
    fround(fround(factor * current_health).max(0.001_f32 as f64))
}

#[must_use]
pub fn create_ether_blast_particle_program(rng: &mut Rng) -> Vec<EtherBlastParticle> {
    let mut particles = Vec::with_capacity(108);
    for heading in (0..720).step_by(20) {
        particles.push(create_particle(rng, f64::from(heading), 11));
        for _ in 0..2 {
            particles.push(create_particle(rng, f64::from(heading), 45));
        }
    }
    particles
}

#[must_use]
pub fn ether_blast_particle_frame(
    particle: EtherBlastParticle,
    age_ticks: f64,
) -> EtherBlastParticleFrame {
    assert!(age_ticks.is_finite() && age_ticks >= 0.0, "particle age must be nonnegative");
    let whole_ticks = age_ticks.floor() as u64;
    let fraction = age_ticks - whole_ticks as f64;
    let mut velocity = particle.velocity;
    let mut position = particle.offset;
    for _ in 0..whole_ticks {
        position = Vec2::new(fround(position.x + velocity.x), fround(position.y + velocity.y));
        velocity =
            Vec2::new(fround(velocity.x * particle.damping), fround(velocity.y * particle.damping));
    }
    position = Vec2::new(
        fround(position.x + velocity.x * fraction),
        fround(position.y + velocity.y * fraction),
    );
    EtherBlastParticleFrame {
        particle,
        alpha: fround(1.0 - age_ticks * particle.alpha_loss).max(0.0),
        position,
    }
}

fn create_particle(rng: &mut Rng, heading: f64, sprite_record: usize) -> EtherBlastParticle {
    let radial_distance = rng.draw_scaled_float(200.0, false);
    let heading_jitter = rng.draw_scaled_float(10.0, true);
    let scale_jitter = rng.draw_scaled_float(4.0, false);
    let mut green = 0.5;
    if sprite_record == 45 {
        green = fround(green + rng.draw_scaled_float(0.8_f32 as f64, false));
    }
    let speed_jitter = rng.draw_scaled_float(5.0, false);
    let alpha_floor = if sprite_record == 11 { 0.1_f32 as f64 } else { 0.25 };
    let alpha_maximum = if sprite_record == 11 { 0.05_f32 as f64 } else { 0.25 };
    let alpha_jitter = rng.draw_scaled_float(alpha_maximum, false);
    let position_direction = direction_from_heading(heading + heading_jitter);
    let velocity_direction = direction_from_heading(heading);
    let speed = if sprite_record == 11 { speed_jitter } else { fround(5.0 + speed_jitter) };
    EtherBlastParticle {
        alpha_loss: fround(0.1_f32 as f64 * fround(alpha_floor + alpha_jitter)),
        blue: 1.0,
        damping: 0.95_f32 as f64,
        green,
        offset: Vec2::new(
            fround(radial_distance * position_direction.x),
            fround(radial_distance * position_direction.y),
        ),
        red: 1.0,
        rotation_degrees: heading,
        scale: fround(1.0 + scale_jitter),
        sprite_record,
        velocity: Vec2::new(
            fround(speed * velocity_direction.x),
            fround(speed * velocity_direction.y),
        ),
    }
}
