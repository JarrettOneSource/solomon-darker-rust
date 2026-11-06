//! The Fire primary's effects: the detonation and its private ember fan, embers, fire
//! patches and friendly Imps. The shared enemy path kernel owns routing.

use super::model::*;
use super::targeting::{PrimarySpellTarget, primary_target_eligible};
use crate::enemies::path::{
    EnemyNavigation, EnemyPathGoalRequest, EnemyPathState, EnemySteeringRequest,
    PathRecoveryRequest, RadialDirection, TangentDirection, build_enemy_steering,
    enemy_steering_goal, enemy_target_refresh_ticks, resolve_enemy_path_goal,
};
use crate::float;
use crate::float::{cos, fround, hypot, sin};
use crate::painter::{PainterRegistration, WorldManagerLane};
use crate::profile::primary::SpentEmber;
use crate::random_domain::{
    random_float_from_word, random_int_from_word, signed_random_float_from_words,
};
use crate::{Rng, Vec2};

pub const FIRE_IMPACT_LIFETIME_TICKS: u64 = 16;
pub const FIRE_EXPLOSION_LIFETIME_TICKS: u64 = 37;

#[must_use]
pub fn fire_particle_fade_step(id: u64, enhanced_effects: bool) -> f64 {
    let base = 0.05 + fire_presentation_random(id, 0, 5, 0.05);
    if enhanced_effects { base * 0.5 } else { base }
}
#[must_use]
pub fn fire_particle_lifetime_ticks(id: u64, enhanced_effects: bool) -> u64 {
    (1.0 / fire_particle_fade_step(id, enhanced_effects)).floor() as u64 + 1
}
#[must_use]
pub fn fire_particle_variant(id: u64) -> u32 {
    fire_presentation_random_int(id, 0, 6, 4)
}
#[must_use]
pub fn fire_impact_pitch(id: u64) -> f64 {
    0.9 + fire_presentation_random(id, 0, 12, 0.2)
}
#[must_use]
pub fn fire_presentation_random(id: u64, sample: u64, channel: u32, maximum: f64) -> f64 {
    random_float_from_word(fire_presentation_word(id, sample, channel), maximum)
}
#[must_use]
pub fn fire_presentation_random_int(id: u64, sample: u64, channel: u32, bound: u32) -> u32 {
    random_int_from_word(fire_presentation_word(id, sample, channel), bound)
}
#[must_use]
pub fn fire_presentation_signed_random(id: u64, sample: u64, channel: u32, maximum: f64) -> f64 {
    signed_random_float_from_words(
        fire_presentation_word(id, sample, channel),
        fire_presentation_word(id, sample, channel ^ 0x7f4a_7c15),
        maximum,
    )
}
fn fire_presentation_word(id: u64, sample: u64, channel: u32) -> u32 {
    let mut value = (id as u32).wrapping_mul(0x9e37_79b1)
        ^ (sample as u32).wrapping_add(1).wrapping_mul(0x85eb_ca6b)
        ^ channel.wrapping_add(1).wrapping_mul(0xc2b2_ae35);
    value = (value ^ (value >> 16)).wrapping_mul(0x21f0_aaad);
    value = (value ^ (value >> 15)).wrapping_mul(0x735a_2d97);
    value ^ (value >> 15)
}
/// What a fireball carries into its detonation: burn, explosion and ember damage, the
/// explosion radius, ember count, private seed and what spent embers become.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FireProjectilePayload {
    pub burn_damage: f64,
    pub ember_damage: f64,
    pub ember_fragments: u32,
    pub explode_damage: f64,
    pub explode_radius: f64,
    pub private_seed: i32,
    pub spent_ember: SpentEmber,
}

impl From<&PrimarySpellFireProjectile> for FireProjectilePayload {
    fn from(spell: &PrimarySpellFireProjectile) -> Self {
        Self {
            burn_damage: spell.burn_damage,
            ember_damage: spell.ember_damage,
            ember_fragments: spell.ember_fragments,
            explode_damage: spell.explode_damage,
            explode_radius: spell.explode_radius,
            private_seed: spell.private_seed,
            spent_ember: spell.spent_ember,
        }
    }
}

/// The native explosion has no manager id until its caller enrolls the effect.
#[derive(Clone, Debug, PartialEq)]
pub struct FireExplosionState {
    pub burn_damage: f64,
    pub damage: f64,
    pub footprint_dimension: f64,
    pub origin: Vec2,
    pub owner_id: String,
    pub presentation: FireExplosionPresentation,
    pub visual_scale: f64,
    pub world_key: String,
}

/// What a spent ember becomes: nothing, an immolation explosion or a friendly Imp.
#[derive(Clone, Debug, PartialEq)]
pub enum FireEmberRetirement {
    None,
    Immolate(FireExplosionState),
    Imp {
        burn_damage: f64,
        damage: f64,
        lifetime_ticks: u64,
        owner_id: String,
        position: Vec2,
        world_key: String,
    },
}

/// The result of one ember tick: whether it stays and what it retires into.
#[derive(Clone, Debug, PartialEq)]
pub struct FireEmberStep {
    pub retained: bool,
    pub retirement: FireEmberRetirement,
}

/// A finished detonation: its explosion, embers, the contacts from the embers'
/// pre-ticks, sound pitch and the next free spell ID.
#[derive(Clone, Debug, PartialEq)]
pub struct FireDetonation {
    pub contacts: Vec<FireRadialContact>,
    pub embers: Vec<PrimarySpellFireEmber>,
    pub explosion: Option<FireExplosionState>,
    pub next_id: u64,
    pub sound_pitch: f64,
}

/// Inputs for creating a fire patch; each `None` takes the patch type's default.
pub struct CreateFirePatchOptions {
    pub atlas_phase_step: Option<f64>,
    pub burn_damage: f64,
    pub damage: f64,
    pub draw_alpha: Option<f64>,
    pub fade_alpha: Option<f64>,
    pub id: u64,
    pub life: Option<f64>,
    pub patch_type: FirePatchType,
    pub owner_id: String,
    pub painter_registration: Option<PainterRegistration>,
    pub position: Vec2,
    pub scale: Option<f64>,
    pub supplemental_contact: bool,
    pub velocity: Option<Vec2>,
    pub velocity_multiplier: Option<Vec2>,
    pub world_key: String,
}

/// Inputs for spawning a friendly Imp: its damage, lifetime, owner and position.
pub struct SpawnFireGoodImpOptions {
    pub burn_damage: f64,
    pub damage: f64,
    pub id: u64,
    pub lifetime_ticks: u64,
    pub owner_id: String,
    pub position: Vec2,
    pub world_key: String,
}

/// What a friendly Imp's step reads from the world: occupancy, navigation and targets.
pub struct FireGoodImpStepContext<'a> {
    pub can_occupy: &'a dyn Fn(Vec2) -> bool,
    pub navigation: Option<&'a dyn EnemyNavigation>,
    pub targets: &'a [PrimarySpellTarget],
}

/// The result of one friendly Imp tick: whether it stays and the contact it made.
pub struct FireGoodImpStep {
    pub contact: Option<FireTargetContact>,
    pub retained: bool,
}

#[must_use]
pub fn create_fire_patch(
    options: CreateFirePatchOptions,
    atlas_phase: f64,
    horizontal_sign: f64,
) -> PrimarySpellFirePatch {
    require_nonnegative(options.burn_damage, "Fire patch Burn damage");
    require_nonnegative(options.damage, "Fire patch damage");
    assert!(options.id > 0, "Fire patch id must be positive");
    let life = options.life.unwrap_or(2.0);
    let scale = options.scale.unwrap_or(1.0);
    let moving = options.patch_type == FirePatchType::Moving;
    let draw_alpha = options.draw_alpha.unwrap_or(if moving { 4.0 } else { 1.0 });
    let atlas_phase_step =
        options.atlas_phase_step.unwrap_or(if moving { 0.12_f32 as f64 } else { 0.25_f32 as f64 });
    assert!(life.is_finite() && life > 0.0, "Fire patch life must be finite and positive");
    assert!(scale.is_finite() && scale > 0.0, "Fire patch scale must be finite and positive");
    require_nonnegative(draw_alpha, "Fire patch draw alpha");
    assert!((0.0..=32.0).contains(&atlas_phase), "Fire patch phase is inside the native bank");
    require_nonnegative(atlas_phase_step, "Fire patch phase step");
    assert!(
        horizontal_sign == -1.0 || horizontal_sign == 1.0,
        "Fire patch horizontal sign is one or negative one"
    );
    let registration = options.painter_registration.unwrap_or(PainterRegistration {
        manager_lane: WorldManagerLane::Actor,
        registration_ordinal: options.id as usize,
    });
    let mut common = transient_base(options.id, options.owner_id, options.world_key);
    common.painter_registrations.push(registration);
    PrimarySpellFirePatch {
        common,
        atlas_phase: fround(atlas_phase),
        atlas_phase_step: fround(atlas_phase_step),
        burn_damage: options.burn_damage,
        damage: options.damage,
        draw_alpha,
        fade_alpha: fround(options.fade_alpha.unwrap_or(0.0)),
        life: fround(life),
        patch_type: options.patch_type,
        position: float32_vector(options.position),
        horizontal_sign,
        scale: fround(scale),
        supplemental_contact: options.supplemental_contact,
        velocity: options.velocity.unwrap_or(Vec2::ZERO),
        velocity_multiplier: options.velocity_multiplier.unwrap_or(Vec2::new(1.0, 1.0)),
    }
}

#[must_use]
pub fn spawn_native_fire_patch(
    options: CreateFirePatchOptions,
    rng: &mut Rng,
) -> PrimarySpellFirePatch {
    let phase = rng.draw_scaled_float(32.0, false);
    let sign = rng.draw_sign(1.0);
    create_fire_patch(options, phase, sign)
}

/// Returns retention and this tick's contact; a retiring patch can still deal damage.
#[must_use]
pub fn step_fire_patch(
    patch: &mut PrimarySpellFirePatch,
    global_tick: u64,
) -> (bool, Option<FireRadialContact>) {
    patch.atlas_phase = positive_modulo(fround(patch.atlas_phase + patch.atlas_phase_step), 32.0);
    patch.life = fround(patch.life - 0.01_f32 as f64);
    patch.fade_alpha = fround(patch.fade_alpha + 0.05_f32 as f64).min(1.0);
    let contact = if patch.damage > 0.0 && global_tick.is_multiple_of(3) {
        Some(FireRadialContact {
            amount: patch.damage * (3.0 / 100.0),
            burn_damage: patch.burn_damage,
            owner_id: patch.common.owner_id.clone(),
            position: patch.position,
            radius: 32.0 * patch.scale,
            spell_id: patch.common.id,
            world_key: patch.common.world_key.clone(),
        })
    } else {
        None
    };
    // The terminal native tick still publishes its contact before the patch retires.
    if patch.life <= 0.0 {
        return (false, contact);
    }
    patch.position = Vec2::new(
        fround(patch.position.x + patch.velocity.x),
        fround(patch.position.y + patch.velocity.y),
    );
    patch.velocity = Vec2::new(
        fround(patch.velocity.x * patch.velocity_multiplier.x),
        fround(patch.velocity.y * patch.velocity_multiplier.y),
    );
    patch.common.age_ticks += 1;
    (true, contact)
}

/// Native Badguy → Imp → GoodImp constructor order. Discarded draws remain authoritative.
#[must_use]
pub fn spawn_fire_good_imp(
    options: SpawnFireGoodImpOptions,
    rng: &mut Rng,
) -> PrimarySpellFireGoodImp {
    require_nonnegative(options.burn_damage, "GoodImp Burn damage");
    require_nonnegative(options.damage, "GoodImp damage");
    assert!(
        options.id > 0 && options.lifetime_ticks > 0,
        "GoodImp id and lifetime must be positive"
    );
    let path = EnemyPathState::new(rng);
    rng.draw_word();
    let body_scale = fround(0.98_f32 as f64 + rng.draw_scaled_float(0.05_f32 as f64, true));
    for _ in 0..6 {
        rng.draw_word();
    }
    let radius = rng.draw_scaled_float(2.5_f32 as f64, false);
    let effect_phase = rng.draw_scaled_float(10.0, false);
    let body_variant = rng.draw_integer(4) as u32;
    let body_rotation_degrees = rng.draw_scaled_float(45.0, true);
    PrimarySpellFireGoodImp {
        common: transient_base(options.id, options.owner_id, options.world_key),
        body_rotation_degrees,
        body_scale,
        body_variant,
        bounce_sound_index: 0,
        bounce_sound_pitch: 1.0,
        bounce_sound_sequence: 0,
        burn_damage: options.burn_damage,
        collision_radius: fround(2.5 - radius),
        contact_age_ticks: None,
        contact_origin: None,
        contact_scale: 1.0,
        contact_sound_index: 0,
        contact_sound_pitch: 1.0,
        contact_sound_sequence: 0,
        damage: options.damage,
        effect_alpha: 0.0,
        effect_phase,
        flight_speed: 4.5_f32 as f64,
        heading_degrees: 0.0,
        light_glow: 0.0,
        next_target_refresh_tick: 0,
        path,
        position: options.position,
        remaining_ticks: options.lifetime_ticks,
        target_id: None,
        vertical_offset: 0.0,
        vertical_velocity: 0.0,
    }
}

#[must_use]
pub fn step_fire_good_imp(
    actor: &mut PrimarySpellFireGoodImp,
    context: FireGoodImpStepContext<'_>,
    rng: &mut Rng,
) -> FireGoodImpStep {
    let (target, next_refresh) = refresh_good_imp_target(actor, context.targets);
    if target.map(|target| target.id.as_str()) != actor.target_id.as_deref() {
        actor.path.clear_route();
    }
    if actor.path.reorientation_ticks_remaining > 0 {
        actor.heading_degrees = actor.path.step_reorientation(
            actor.heading_degrees,
            actor.position,
            target.map(|target| target.position),
        );
    } else if actor.common.age_ticks.is_multiple_of(10) {
        let mut request = EnemySteeringRequest {
            actor_heading_deg: actor.heading_degrees,
            actor_position: actor.position,
            cadence_ticks: 10,
            full_movement: true,
            goal_position: None,
            movement_per_tick: fround(actor.flight_speed * 0.25 * actor.path.speed_factor),
            radial_direction: RadialDirection::Approach,
            status_factor: 1.0,
            tangent_direction: TangentDirection::Hold,
            target_heading_deg: target.and_then(|target| target.heading_degrees).unwrap_or(0.0),
            target_position: target.map(|target| target.position),
        };
        let mut goal = enemy_steering_goal(&actor.path, &request);
        if let Some(navigation) = context.navigation {
            let routed = resolve_enemy_path_goal(
                actor.path,
                EnemyPathGoalRequest {
                    actor_position: actor.position,
                    body_radius: actor.collision_radius,
                    cadence_ticks: 10,
                    navigation_clearance: 25.0,
                    raw_goal: goal,
                    target_position: target.map(|target| target.position),
                    target_refresh_ticks: enemy_target_refresh_ticks(1),
                },
                navigation,
            );
            actor.path = routed.state;
            if routed.turn_around {
                request.actor_heading_deg =
                    positive_modulo(request.actor_heading_deg + 180.0, 360.0);
            }
            goal = routed.goal;
        }
        request.goal_position = Some(goal);
        let steering = build_enemy_steering(actor.path, request);
        let old_position = actor.position;
        let requested = Vec2::new(
            fround(old_position.x + steering.delta.x),
            fround(old_position.y + steering.delta.y),
        );
        if (context.can_occupy)(requested) {
            actor.position = requested;
        }
        actor.heading_degrees = steering.heading_deg;
        actor.path = steering.state;
        actor.path.step_recovery(
            rng,
            PathRecoveryRequest {
                flanking_enabled: true,
                requested_distance: hypot(steering.delta.x, steering.delta.y),
                status_factor: 1.0,
                tick: actor.common.age_ticks,
                traveled_distance: hypot(
                    actor.position.x - old_position.x,
                    actor.position.y - old_position.y,
                ),
            },
        );
    }
    actor.effect_phase =
        positive_modulo(fround(actor.effect_phase + actor.flight_speed.abs() * 0.25), 10.0);
    actor.vertical_offset = fround(actor.vertical_offset + actor.vertical_velocity);
    actor.vertical_velocity = fround(actor.vertical_velocity + 0.4);
    actor.effect_alpha = fround(actor.effect_alpha - 0.05).max(0.0);
    actor.contact_age_ticks =
        actor.contact_age_ticks.and_then(|age| (age + 1 < 16).then_some(age + 1));
    if actor.contact_age_ticks.is_none() {
        actor.contact_origin = None;
    }
    let mut contact = None;
    if actor.vertical_offset > 0.0 {
        actor.vertical_offset = 0.0;
        actor.flight_speed =
            fround((1.0 + rng.draw_scaled_float(1.5_f32 as f64, false)) * 4.5_f32 as f64);
        actor.bounce_sound_pitch = fround(1.0 + rng.draw_scaled_float(0.1_f32 as f64, false));
        actor.bounce_sound_index = rng.draw_integer(8) as u32;
        actor.bounce_sound_sequence += 1;
        actor.vertical_velocity = fround(-(3.0 + rng.draw_scaled_float(3.0, false)));
        actor.body_variant = rng.draw_integer(4) as u32;
        actor.body_rotation_degrees = rng.draw_scaled_float(60.0, true);
        if rng.draw_integer(20) == 3 {
            actor.vertical_velocity = fround(actor.vertical_velocity * 1.5);
        }
        actor.effect_alpha = 1.0;
        if let Some(target) = target {
            let reach = fround((target.body_radius + 45.0) * 1.25);
            if hypot(actor.position.x - target.position.x, actor.position.y - target.position.y)
                <= reach
            {
                actor.contact_sound_pitch =
                    fround(1.0 + rng.draw_scaled_float(0.25_f32 as f64, false));
                actor.contact_sound_index = rng.draw_integer(3) as u32;
                actor.contact_sound_sequence += 1;
                actor.contact_scale = fround(0.5 + rng.draw_scaled_float(0.1_f32 as f64, false));
                actor.heading_degrees = positive_modulo(
                    actor.heading_degrees + 180.0 + rng.draw_scaled_float(45.0, false),
                    360.0,
                );
                rng.draw_scaled_float(0.6_f32 as f64, false);
                rng.draw_scaled_float(0.5_f32 as f64, false);
                let radians = actor.heading_degrees * std::f64::consts::PI / 180.0;
                actor.contact_origin = Some(Vec2::new(
                    fround(actor.position.x + float::sin(radians) * 15.0),
                    fround(actor.position.y - float::cos(radians) * 15.0 - 15.0),
                ));
                actor.contact_age_ticks = Some(0);
                contact = Some(FireTargetContact {
                    amount: actor.damage,
                    owner_id: actor.common.owner_id.clone(),
                    spell_id: actor.common.id,
                    target_id: target.id.clone(),
                    world_key: actor.common.world_key.clone(),
                });
            }
        }
    }
    actor.remaining_ticks =
        actor.remaining_ticks.saturating_sub(if target.is_none() { 2 } else { 1 });
    if actor.remaining_ticks == 0 {
        return FireGoodImpStep { contact, retained: false };
    }
    actor.common.age_ticks += 1;
    actor.light_glow = fround(actor.light_glow + 0.01).min(1.0);
    actor.next_target_refresh_tick = next_refresh;
    actor.target_id = target.map(|target| target.id.clone());
    FireGoodImpStep { contact, retained: true }
}

pub fn draw_fire_private_seed(rng: &mut Rng) -> i32 {
    rng.draw_integer(1_000_001)
}

#[must_use]
pub fn fire_direct_damage(base_damage: f64, explode_damage: f64) -> f64 {
    require_nonnegative(base_damage, "Fireball base damage");
    require_nonnegative(explode_damage, "Fireball explosion damage");
    (if explode_damage > 0.0 { base_damage - explode_damage } else { base_damage }).max(0.0)
}

/// The constructor fan is resumable at its observer boundaries: explosion,
/// Ember registration, then each of its ten immediate receiver contacts. It owns
/// only private fan state; the caller still passes the one shared gameplay RNG.
pub struct FireEmberFan<'a> {
    first_id: u64,
    index: u32,
    payload: FireProjectilePayload,
    origin: Vec2,
    owner_id: &'a str,
    world_key: &'a str,
    private_rng: Rng,
    start_heading: f64,
    heading_step: f64,
}

/// A detonation's start: its optional explosion, sound pitch and the ember fan still to
/// be constructed.
pub struct FireDetonationBirth<'a> {
    pub explosion: Option<FireExplosionState>,
    pub sound_pitch: f64,
    pub fan: FireEmberFan<'a>,
}

pub fn begin_fire_detonation<'a>(
    first_id: u64,
    payload: FireProjectilePayload,
    origin: Vec2,
    owner_id: &'a str,
    world_key: &'a str,
    rng: &mut Rng,
) -> FireDetonationBirth<'a> {
    assert!(first_id > 0, "Fire detonation first id must be positive");
    let mut private_rng = Rng::new(i64::from(payload.private_seed));
    let sound_pitch = fround(1.0 + rng.draw_scaled_float(0.1, true));
    let start_heading = private_rng.draw_scaled_float(360.0, false);
    let explosion = if payload.explode_radius > 0.0 && payload.explode_damage > 0.0 {
        let visual_scale = fround((payload.explode_radius - 10.0) * 0.18 + 1.0);
        Some(FireExplosionState {
            burn_damage: payload.burn_damage,
            damage: payload.explode_damage * 0.5,
            footprint_dimension: visual_scale * 110.0,
            origin,
            owner_id: owner_id.to_owned(),
            presentation: FireExplosionPresentation::Fire,
            visual_scale,
            world_key: world_key.to_owned(),
        })
    } else {
        None
    };
    FireDetonationBirth {
        explosion,
        sound_pitch,
        fan: FireEmberFan {
            first_id,
            index: 0,
            payload,
            origin,
            owner_id,
            world_key,
            private_rng,
            start_heading,
            heading_step: if payload.ember_fragments == 0 {
                0.0
            } else {
                360.0 / f64::from(payload.ember_fragments)
            },
        },
    }
}

impl FireEmberFan<'_> {
    pub fn next_ember(&mut self, rng: &mut Rng) -> Option<PrimarySpellFireEmber> {
        if self.index >= self.payload.ember_fragments {
            return None;
        }
        let phase = rng.draw_scaled_float(4.0, false);
        // Constructor vertical velocity is overwritten by the private fan draw.
        rng.draw_scaled_float(3.0, false);
        let cadence = rng.draw_integer(10) as u64;
        let jitter = self.private_rng.draw_scaled_float(self.heading_step / 3.0, true);
        let speed_random = self.private_rng.draw_scaled_float(0.5, false);
        let vertical = self.private_rng.draw_scaled_float(3.0, false);
        let heading = (self.start_heading + f64::from(self.index) * self.heading_step + jitter)
            * std::f64::consts::PI
            / 180.0;
        let speed = fround((1.5 + speed_random) * 0.75);
        let velocity = Vec2::new(fround(cos(heading) * speed), fround(sin(heading) * speed));
        let ember = PrimarySpellFireEmber {
            common: transient_base(
                self.first_id + u64::from(self.index),
                self.owner_id.to_owned(),
                self.world_key.to_owned(),
            ),
            burn_damage: self.payload.burn_damage,
            contact_cadence: cadence,
            contact_due: false,
            damage: self.payload.ember_damage,
            height: -6.0,
            horizontal_velocity: velocity,
            life: 3.0,
            phase,
            position: Vec2::new(
                fround(self.origin.x + velocity.x * 10.0),
                fround(self.origin.y + velocity.y * 10.0),
            ),
            spent_ember: self.payload.spent_ember,
            vertical_velocity: fround(-(2.0 + vertical)),
        };
        self.index += 1;
        Some(ember)
    }
}

#[must_use]
pub fn create_fire_detonation(
    first_id: u64,
    payload: FireProjectilePayload,
    origin: Vec2,
    owner_id: &str,
    world_key: &str,
    rng: &mut Rng,
) -> FireDetonation {
    let mut birth = begin_fire_detonation(first_id, payload, origin, owner_id, world_key, rng);
    let mut embers = Vec::with_capacity(payload.ember_fragments as usize);
    let mut contacts = Vec::new();
    while let Some(mut ember) = birth.fan.next_ember(rng) {
        for _ in 0..10 {
            assert!(
                step_fire_ember(&mut ember).retained,
                "a newborn Ember survives its ten native pre-ticks"
            );
            if ember.contact_due {
                contacts.push(fire_ember_contact(&ember));
            }
        }
        ember.contact_due = false;
        embers.push(ember);
    }
    FireDetonation {
        contacts,
        embers,
        explosion: birth.explosion,
        next_id: first_id + u64::from(payload.ember_fragments),
        sound_pitch: birth.sound_pitch,
    }
}

#[must_use]
pub fn step_fire_ember(ember: &mut PrimarySpellFireEmber) -> FireEmberStep {
    if ember.vertical_velocity != 0.0 {
        let movement_factor = ember.vertical_velocity.abs().min(1.0);
        ember.position = Vec2::new(
            fround(ember.position.x + ember.horizontal_velocity.x * movement_factor),
            fround(ember.position.y + ember.horizontal_velocity.y * movement_factor),
        );
        ember.height = fround(ember.height + ember.vertical_velocity);
        ember.vertical_velocity = fround(ember.vertical_velocity + 0.15_f32 as f64);
        if ember.height > 0.0 {
            ember.height = 0.0;
            ember.vertical_velocity = fround(ember.vertical_velocity * -0.5);
            ember.horizontal_velocity = Vec2::new(
                fround(ember.horizontal_velocity.x * 0.5),
                fround(ember.horizontal_velocity.y * 0.5),
            );
            if ember.vertical_velocity >= -0.5 {
                ember.vertical_velocity = 0.0;
            }
        }
    } else {
        ember.life = fround(ember.life - 0.015_f32 as f64);
    }
    if ember.life > 1.0 {
        ember.phase = positive_modulo(fround(ember.phase + 0.25), 4.0);
    }
    ember.contact_cadence += 1;
    ember.contact_due = ember.contact_cadence > 3;
    if ember.contact_due {
        ember.contact_cadence = 0;
    }
    ember.common.age_ticks += 1;
    if ember.vertical_velocity != 0.0 || ember.life >= 1.0 {
        return FireEmberStep { retained: true, retirement: FireEmberRetirement::None };
    }
    let retirement = match ember.spent_ember {
        SpentEmber::Immolate { damage } => FireEmberRetirement::Immolate(FireExplosionState {
            burn_damage: ember.burn_damage,
            damage: damage * 0.5,
            footprint_dimension: 110.0,
            origin: ember.position,
            owner_id: ember.common.owner_id.clone(),
            presentation: FireExplosionPresentation::Fire,
            visual_scale: 1.0,
            world_key: ember.common.world_key.clone(),
        }),
        SpentEmber::Imp { damage, lifetime_ticks } => FireEmberRetirement::Imp {
            burn_damage: ember.burn_damage,
            damage: damage * 0.5,
            lifetime_ticks,
            owner_id: ember.common.owner_id.clone(),
            position: ember.position,
            world_key: ember.common.world_key.clone(),
        },
        SpentEmber::None => FireEmberRetirement::None,
    };
    let retired = !matches!(retirement, FireEmberRetirement::None) || ember.life <= 0.0;
    FireEmberStep { retained: !retired, retirement }
}

#[must_use]
pub fn fire_ember_contact(ember: &PrimarySpellFireEmber) -> FireRadialContact {
    FireRadialContact {
        amount: ember.damage,
        burn_damage: ember.burn_damage,
        owner_id: ember.common.owner_id.clone(),
        position: ember.position,
        radius: 7.0,
        spell_id: ember.common.id,
        world_key: ember.common.world_key.clone(),
    }
}

fn refresh_good_imp_target<'a>(
    source: &PrimarySpellFireGoodImp,
    targets: &'a [PrimarySpellTarget],
) -> (Option<&'a PrimarySpellTarget>, u64) {
    let current = source.target_id.as_ref().and_then(|id| {
        targets.iter().find(|target| target.id == *id && primary_target_eligible(target, 0x2))
    });
    if current.is_some() && source.common.age_ticks < source.next_target_refresh_tick {
        return (current, source.next_target_refresh_tick);
    }
    let mut selected: Option<&PrimarySpellTarget> = None;
    let mut selected_distance = f64::INFINITY;
    for target in targets {
        if !primary_target_eligible(target, 0x2) {
            continue;
        }
        let distance =
            hypot(target.position.x - source.position.x, target.position.y - source.position.y);
        if distance < selected_distance
            || (distance == selected_distance
                && selected
                    .is_some_and(|prior| target.registration_order < prior.registration_order))
        {
            selected = Some(target);
            selected_distance = distance;
        }
    }
    (selected, source.common.age_ticks + enemy_target_refresh_ticks(1))
}

fn transient_base(id: u64, owner_id: String, world_key: String) -> PrimarySpellTransientBase {
    PrimarySpellTransientBase {
        age_ticks: 0,
        id,
        light_registration: None,
        owner_id,
        painter_registrations: Vec::new(),
        world_key,
    }
}
fn float32_vector(vector: Vec2) -> Vec2 {
    Vec2::new(fround(vector.x), fround(vector.y))
}
fn positive_modulo(value: f64, period: f64) -> f64 {
    ((value % period) + period) % period
}
fn require_nonnegative(value: f64, field: &str) {
    assert!(value.is_finite() && value >= 0.0, "{field} must be finite and nonnegative");
}
