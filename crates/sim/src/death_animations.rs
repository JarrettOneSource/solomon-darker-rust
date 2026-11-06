//! Retained Golem death fragments and the weapons dropped by dying players. Both bounce with
//! `step_bouncer_motion`, drawing from the secondary simulation's RNG.

use crate::appearance::equipment::PlayerWeaponAppearance;
use crate::float::{cos, fround, sin};
use crate::painter::PainterRegistration;
use crate::{Rng, Vec2};

pub const GOLEM_DEATH_MAX_AGE_ENHANCED: u64 = 710;
pub const GOLEM_DEATH_MAX_AGE_BASIC: u64 = 177;
pub const GOLEM_DEATH_FRAGMENT_COUNT: usize = 30;
pub const GOLEM_DEATH_PAINTER_COUNT: usize = GOLEM_DEATH_FRAGMENT_COUNT + 1;
pub const GOLEM_DEATH_STAR_TICKS: u64 = 15;

/// A bouncing object's ground position, height, velocities and spin.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BouncerMotion {
    pub bounce_velocity: f64,
    pub height: f64,
    pub position: Vec2,
    pub rotation_degrees: f64,
    pub rotation_step_degrees: f64,
    pub velocity: Vec2,
    pub vertical_velocity: f64,
}

/// The random stream a bouncing object's owner lends to its motion. Enemy world effects
/// use xorshift; Golem fragments and player weapons use the secondary simulation's `Rng`.
pub trait BouncerRandom {
    fn float(&mut self, maximum: f64) -> f64;
    fn integer(&mut self, maximum: u32) -> u32;
}
impl BouncerRandom for Rng {
    fn float(&mut self, maximum: f64) -> f64 {
        self.draw_scaled_float(maximum, false)
    }
    fn integer(&mut self, maximum: u32) -> u32 {
        self.draw_integer(maximum as i32) as u32
    }
}

/// Returns whether airborne motion skipped this tick; it pauses on every third tick. A
/// grounded fragment still runs its owner's lifetime subtraction, without another random draw.
pub fn step_bouncer_motion(
    motion: &mut BouncerMotion,
    tick: u64,
    rng: &mut impl BouncerRandom,
) -> bool {
    if motion.height == 0.0 {
        return false;
    }
    if tick.is_multiple_of(3) {
        return true;
    }
    motion.position = Vec2::new(
        fround(motion.position.x + motion.velocity.x),
        fround(motion.position.y + motion.velocity.y),
    );
    let mut height = fround(motion.height + motion.vertical_velocity);
    motion.vertical_velocity = fround(motion.vertical_velocity + 0.4);
    // Exact-plane landings are admitted too; a strict positive test strands motion at zero.
    if height >= 0.0 {
        motion.rotation_step_degrees = fround(1.0 + rng.float(10.0));
        motion.bounce_velocity = fround(motion.bounce_velocity * (0.65_f32 as f64));
        motion.vertical_velocity = motion.bounce_velocity;
        if rng.integer(2) == 1 {
            motion.velocity =
                Vec2::new(fround(motion.velocity.x * 0.65), fround(motion.velocity.y * 0.65));
        }
        if motion.vertical_velocity > -0.75 {
            motion.bounce_velocity = 0.0;
            motion.vertical_velocity = 0.0;
            motion.velocity = Vec2::ZERO;
            motion.rotation_step_degrees = 0.0;
        }
        height = motion.vertical_velocity;
    }
    motion.height = height;
    motion.rotation_degrees = fround(motion.rotation_degrees + motion.rotation_step_degrees);
    false
}

/// One Golem death fragment: its remaining life and bouncing motion.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DeathFragment {
    pub life: f64,
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub motion: BouncerMotion,
}

/// A dead Golem's fragments (`None` once expired) and the rotation of its star.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GolemDeathAnimation {
    pub fragments: Vec<Option<DeathFragment>>,
    pub star_rotation_degrees: f64,
    pub star_step_degrees: f64,
}

#[must_use]
pub fn create_golem_death_animation(
    rng: &mut Rng,
    position: Vec2,
    enhanced: bool,
) -> GolemDeathAnimation {
    let mut angles: [f64; GOLEM_DEATH_FRAGMENT_COUNT] =
        std::array::from_fn(|index| index as f64 * 18.0);
    for index in 0..angles.len() {
        let selected = rng.draw_integer(angles.len() as i32) as usize;
        angles.swap(index, selected);
    }
    let mut fragments = Vec::with_capacity(angles.len());
    for angle in angles {
        let bounce_velocity = fround(-(rng.draw_scaled_float(3.0, false) + 2.0));
        let height = fround(-rng.draw_scaled_float(20.0, false));
        let rotation_degrees = rng.draw_scaled_float(360.0, false);
        rng.draw_scaled_float(10.0, false); // The death recipe overwrites the base constructor spin.
        let magnitude = fround(1.5 * (rng.draw_scaled_float(1.0, false) + 0.5));
        let radius = fround(rng.draw_scaled_float(10.0, false) + 17.0);
        let rotation_step_degrees = rng.draw_scaled_float(20.0, true);
        let radians = angle * std::f64::consts::PI / 180.0;
        let velocity =
            Vec2::new(fround(sin(radians) * magnitude), fround(-cos(radians) * magnitude));
        fragments.push(Some(DeathFragment {
            life: if enhanced { 10.0 } else { 2.0 },
            motion: BouncerMotion {
                bounce_velocity,
                height,
                position: Vec2::new(
                    fround(position.x + velocity.x * radius),
                    fround(position.y + velocity.y * radius),
                ),
                rotation_degrees,
                rotation_step_degrees,
                velocity,
                vertical_velocity: bounce_velocity,
            },
        }));
    }
    let star_rotation_degrees = rng.draw_scaled_float(360.0, false);
    let star_pitch = rng.draw_scaled_float(5.0, false);
    let star_sample = rng.draw_integer(10);
    GolemDeathAnimation {
        fragments,
        star_rotation_degrees,
        star_step_degrees: fround((f64::from(star_sample) + star_pitch) * 0.5),
    }
}

pub fn step_golem_death_animation(animation: &mut GolemDeathAnimation, tick: u64, rng: &mut Rng) {
    for slot in &mut animation.fragments {
        let Some(fragment) = slot else { continue };
        let skipped = step_bouncer_motion(&mut fragment.motion, tick, rng);
        if !skipped {
            fragment.life = fround(fragment.life - 0.015);
        }
        if fragment.life <= 0.0 {
            *slot = None;
        }
    }
}

/// The weapon a dying player drops: its owner, death epoch, painter registration, appearance
/// and bouncing motion.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DeathWeaponActor {
    pub age_ticks: u64,
    pub birth_tick: u64,
    pub death_epoch: u64,
    pub id: u64,
    pub life: f64,
    pub motion: BouncerMotion,
    pub owner_id: String,
    pub painter_registration: PainterRegistration,
    pub weapon: PlayerWeaponAppearance,
}

/// The dying player's values needed to create a dropped weapon: identity, heading, position,
/// painter registration and weapon appearance.
pub struct DeathWeaponBirth<'a> {
    pub death_epoch: u64,
    pub heading_index: usize,
    pub id: u64,
    pub owner_id: &'a str,
    pub painter_registration: PainterRegistration,
    pub position: Vec2,
    pub tick: u64,
    pub weapon: PlayerWeaponAppearance,
}

#[must_use]
pub fn create_death_weapon_actor(options: DeathWeaponBirth<'_>, rng: &mut Rng) -> DeathWeaponActor {
    let bounce_velocity = fround(-(2.0 + rng.draw_scaled_float(3.0, false)));
    let height = fround(-rng.draw_scaled_float(20.0, false));
    let rotation_degrees = rng.draw_scaled_float(360.0, false);
    let rotation_step_degrees = fround(1.0 + rng.draw_scaled_float(10.0, false));
    let radius = fround(15.0 + rng.draw_scaled_float(10.0, false));
    // The native dropped-weapon recipe deliberately uses the body bank, not +0x6C.
    let radians = options.heading_index as f64 * 15.0 * std::f64::consts::PI / 180.0;
    let velocity = Vec2::new(fround(sin(radians) * 1.5), fround(-cos(radians) * 1.5));
    DeathWeaponActor {
        age_ticks: 0,
        birth_tick: options.tick,
        death_epoch: options.death_epoch,
        id: options.id,
        life: 99999.0,
        motion: BouncerMotion {
            bounce_velocity,
            height,
            position: Vec2::new(
                fround(options.position.x + velocity.x * (radius + 2.0)),
                fround(options.position.y + velocity.y * radius),
            ),
            rotation_degrees,
            rotation_step_degrees,
            velocity,
            vertical_velocity: bounce_velocity,
        },
        owner_id: options.owner_id.into(),
        painter_registration: options.painter_registration,
        weapon: options.weapon,
    }
}

pub fn step_death_weapon_actors(actors: &mut Vec<DeathWeaponActor>, tick: u64, rng: &mut Rng) {
    actors.retain_mut(|actor| {
        let skipped = step_bouncer_motion(&mut actor.motion, tick, rng);
        if !skipped {
            actor.life = fround(actor.life - 0.015);
        }
        if actor.life <= 0.0 {
            return false;
        }
        actor.age_ticks += 1;
        true
    });
}
