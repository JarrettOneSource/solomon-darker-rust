//! Native elemental projectile births and motion from `primary-spells.ts`.

use super::casting::{NativePlayerWeaponKind, primary_spell_emitter_offset};
use super::earth::earth_boulder_flight_orientation_step;
use super::fire::draw_native_fire_private_seed;
use super::model::*;
use super::targeting::*;
use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::hub::HubWizard;
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::profile::WizardElement;
use crate::profile::primary::{
    NativePrimarySkillProfile, NativePrimarySkillProfileBase, NativeSpentEmber,
};
use crate::{NativeRng, Vec2};

pub(super) fn emitter(
    wizard: &HubWizard,
    element: WizardElement,
    weapon: Option<NativePlayerWeaponKind>,
) -> Vec2 {
    let offset = primary_spell_emitter_offset(
        wizard.heading_index,
        wizard.primary_cast.action_tick,
        wizard.primary_cast.channel_active,
        element,
        weapon,
    );
    Vec2::new(wizard.position.x + offset.x, wizard.position.y + offset.y)
}

pub(super) struct OneShotBirth<'a> {
    pub first_id: u64,
    pub owner_id: &'a str,
    pub wizard: &'a HubWizard,
    pub weapon: Option<NativePlayerWeaponKind>,
    pub profile: &'a NativePrimarySkillProfile,
    pub world_key: &'a str,
    pub targets: &'a [PrimarySpellTarget],
    pub underpowered: bool,
}

pub(super) fn create_one_shot_projectiles(
    birth: OneShotBirth<'_>,
    rng: &mut NativeRng,
    order: &mut WorldPainterOrder,
) -> Vec<PrimarySpellProjectile> {
    if let NativePrimarySkillProfile::Weld(profile) = birth.profile {
        let origin = emitter(birth.wizard, WizardElement::Fire, birth.weapon);
        return super::weld::spawn_native_weld_one_shot(
            super::weld::SpawnNativeWeldOneShotInput {
                aim_direction: birth.wizard.primary_cast.aim_direction,
                first_id: birth.first_id,
                origin,
                owner_id: birth.owner_id,
                primary_skill: profile,
                targets: birth.targets,
                underpowered: birth.underpowered,
                world_key: birth.world_key,
            },
            rng,
            Some(order),
        )
        .into_iter()
        .map(PrimarySpellProjectile::Weld)
        .collect();
    }
    let damage = draw_inclusive_damage(birth.profile.common(), rng)
        * if birth.underpowered { 0.5 } else { 1.0 };
    let aim = birth.wizard.primary_cast.aim_direction;
    if let NativePrimarySkillProfile::Fire(profile) = birth.profile {
        let private_seed = draw_native_fire_private_seed(rng);
        let origin = emitter(birth.wizard, WizardElement::Fire, birth.weapon);
        let position = Vec2::new(origin.x + aim.x * 20.0, origin.y + 10.0 + aim.y * 20.0);
        let common = PrimarySpellProjectileBase {
            age_ticks: 0,
            charge: 1.0,
            damage,
            direction: aim,
            flight_ticks: 0,
            id: birth.first_id,
            registration: order.register(WorldManagerLane::Actor),
            owner_id: birth.owner_id.into(),
            phase: PrimarySpellProjectilePhase::Flight,
            position,
            velocity: Vec2::new(aim.x * 4.5, aim.y * 4.5),
            world_key: birth.world_key.into(),
        };
        return vec![PrimarySpellProjectile::Fire(PrimarySpellFireProjectile {
            common,
            burn_damage: if birth.underpowered { 0.0 } else { profile.burn_damage },
            ember_damage: if birth.underpowered { 0.0 } else { profile.ember_damage },
            ember_fragments: if birth.underpowered { 0 } else { profile.ember_fragments },
            explode_damage: if birth.underpowered { 0.0 } else { profile.explode_damage },
            explode_radius: if birth.underpowered { 0.0 } else { profile.explode_radius },
            private_seed,
            spent_ember: if birth.underpowered {
                NativeSpentEmber::None
            } else {
                profile.spent_ember
            },
            underpowered: birth.underpowered,
        })];
    }
    let NativePrimarySkillProfile::Ether(profile) = birth.profile else {
        unreachable!("one-shot elemental profile");
    };
    let aim_heading = heading_degrees_from_vector(aim.x, aim.y);
    let quantity = if birth.underpowered { 1 } else { profile.quantity } as usize;
    let speed =
        if birth.underpowered { 2.4_f32 as f64 } else { fround(3.0 * profile.speed_factor) };
    let origin = emitter(birth.wizard, WizardElement::Ether, birth.weapon);
    let position = Vec2::new(origin.x, origin.y + 10.0);
    let mut projectiles = Vec::with_capacity(quantity);
    for index in 0..quantity {
        let heading_degrees = native_missile_fan_heading(aim_heading, quantity, index);
        let direction = direction_from_heading(heading_degrees);
        let target = select_ether_primary_target(position, direction, birth.targets);
        let common = PrimarySpellProjectileBase {
            age_ticks: 0,
            charge: 1.0,
            damage,
            direction,
            flight_ticks: 0,
            id: birth.first_id + index as u64,
            registration: order.register(WorldManagerLane::Actor),
            owner_id: birth.owner_id.into(),
            phase: PrimarySpellProjectilePhase::Flight,
            position,
            velocity: Vec2::new(direction.x * speed, direction.y * speed),
            world_key: birth.world_key.into(),
        };
        projectiles.push(PrimarySpellProjectile::Ether(PrimarySpellEtherProjectile {
            common,
            damage_retention: if birth.underpowered { 1.0 } else { profile.damage_retention },
            heading_degrees,
            pierces_remaining: if birth.underpowered { 0 } else { profile.pierces },
            reacquires_target: !birth.underpowered && profile.reacquires_target,
            speed,
            target_id: target.map(|target| target.id.clone()),
            turn_input: if birth.underpowered {
                1.2_f32 as f64
            } else {
                fround(2.0 * profile.speed_factor * native_missile_fan_turn_scale(index))
            },
            turn_accumulator: ETHER_PRIMARY_INITIAL_TURN,
            underpowered: birth.underpowered,
            visual_scale: 1.0,
        }));
    }
    projectiles
}

fn draw_inclusive_damage(profile: &NativePrimarySkillProfileBase, rng: &mut NativeRng) -> f64 {
    let count = profile.damage_roll_count;
    assert!(
        count.is_finite() && count.fract() == 0.0 && count >= 1.0,
        "native damage roll count must be a positive integer"
    );
    if count == 1.0 {
        return profile.damage_minimum;
    }
    let draw = f64::from(rng.draw_integer(count as i32));
    profile.damage_minimum
        + draw * (profile.damage_maximum - profile.damage_minimum) / (count - 1.0)
}

pub(super) fn advance_projectile(
    mut spell: PrimarySpellProjectile,
    targets: &[PrimarySpellTarget],
) -> PrimarySpellProjectile {
    if spell.common().phase == PrimarySpellProjectilePhase::Held {
        spell.common_mut().age_ticks += 1;
        return spell;
    }
    match &mut spell {
        PrimarySpellProjectile::Weld(projectile) => {
            super::weld::step_native_weld_projectile(projectile, targets);
            return spell;
        }
        PrimarySpellProjectile::Ether(spell) => {
            let advanced = advance_ether_primary_tracking(EtherPrimaryTrackingInput {
                heading_degrees: spell.heading_degrees,
                movement_scalar: 1.0,
                position: spell.common.position,
                reacquires_target: spell.reacquires_target,
                speed: spell.speed,
                target_id: spell.target_id.as_deref(),
                targets,
                turn_input: spell.turn_input,
                turn_accumulator: spell.turn_accumulator,
            });
            spell.common.direction = advanced.movement.direction;
            spell.common.position = advanced.movement.position;
            spell.heading_degrees = advanced.movement.heading_degrees;
            spell.reacquires_target = advanced.reacquires_target;
            spell.target_id = advanced.target_id;
            spell.turn_accumulator = advanced.movement.turn_accumulator;
            spell.common.velocity = Vec2::new(
                fround(spell.common.direction.x * spell.speed),
                fround(spell.common.direction.y * spell.speed),
            );
        }
        PrimarySpellProjectile::Earth(spell) => {
            let position = Vec2::new(
                fround(spell.common.position.x + spell.common.velocity.x),
                fround(spell.common.position.y + spell.common.velocity.y),
            );
            let stored_delta = Vec2::new(
                fround(position.x - spell.common.position.x),
                fround(position.y - spell.common.position.y),
            );
            spell.orientation = earth_boulder_flight_orientation_step(
                spell.orientation,
                spell.common.direction,
                stored_delta,
                spell.common.charge,
            );
            spell.common.position = position;
        }
        PrimarySpellProjectile::Fire(spell) => {
            // Unlike Earth and Ether, the Website keeps this position in doubles.
            spell.common.position.x += spell.common.velocity.x;
            spell.common.position.y += spell.common.velocity.y;
        }
    }
    spell.common_mut().age_ticks += 1;
    spell.common_mut().flight_ticks += 1;
    spell
}
