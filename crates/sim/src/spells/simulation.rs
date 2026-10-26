//! Shared elemental and welded primaries, preserving the Website actor and player order.
//! Movement commits before this step; damage/contact acceptance belongs to the world.

use crate::spells::transient_storage::{PrimaryTransientRef, WeldWorldRef};

use super::casting::*;
use super::earth::*;
use super::effects::*;
use super::ether::*;
use super::model::*;
use super::projectiles::{OneShotBirth, advance_projectile, create_one_shot_projectiles, emitter};
use super::targeting::*;
use super::water::*;
use super::weld::*;
use crate::enemies::path::EnemyNavigation;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::hub::HubWizard;
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::profile::WizardElement;
use crate::profile::primary::{NativeEarthPrimarySkillProfile, NativePrimarySkillProfile};
use crate::profile::weld::NativeWeldCastKind;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy)]
pub struct PrimarySpellCastAuthority<'a> {
    pub weapon_kind: Option<NativePlayerWeaponKind>,
    pub alive: bool,
    pub available_mana: f64,
    pub cast_progress_factor: f64,
    pub eligible: bool,
    pub plane_active: bool,
    pub primary_skill: &'a NativePrimarySkillProfile,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PrimarySpellInput {
    pub aim: Option<Vec2>,
    pub primary: bool,
}

pub type PrimarySpellObstruction<'a> = dyn Fn(Vec2, Vec2, Option<&str>, u32) -> Option<Vec2> + 'a;

/// The native traversal callback accepts ordinary projectiles and the released
/// Ethereal Boulder. Borrow the real actor so collision keeps its family rules.
#[derive(Clone, Copy)]
pub enum PrimarySpellTraversalActor<'a> {
    Projectile(PrimaryProjectileRef<'a>),
    EtherealBoulder(&'a NativeWeldEtherealBoulderState),
}

#[must_use]
pub fn native_primary_birth_terrain_exclusion_mask(spell: &impl PrimaryProjectileRead) -> u32 {
    match spell.view() {
        PrimaryProjectileRef::Fire(_) => NATIVE_PRIMARY_FLIGHT_TERRAIN_EXCLUSION_MASK,
        PrimaryProjectileRef::Ether(_) => NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK,
        PrimaryProjectileRef::Weld(spell) if matches!(spell.build_id, 1000..=1002) => {
            NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK
        }
        _ => 0,
    }
}

#[must_use]
pub fn native_primary_flight_terrain_exclusion_mask(spell: PrimarySpellTraversalActor<'_>) -> u32 {
    match spell {
        PrimarySpellTraversalActor::Projectile(
            PrimaryProjectileRef::Fire(_) | PrimaryProjectileRef::Ether(_),
        ) => NATIVE_PRIMARY_FLIGHT_TERRAIN_EXCLUSION_MASK,
        PrimarySpellTraversalActor::Projectile(PrimaryProjectileRef::Weld(spell))
            if matches!(spell.build_id, 1000..=1002) =>
        {
            NATIVE_PRIMARY_FLIGHT_TERRAIN_EXCLUSION_MASK
        }
        _ => 0,
    }
}

#[derive(Clone, Copy)]
pub struct PrimarySpellWorldContext<'a> {
    pub tick: u64,
    pub enhanced_effects: bool,
    pub flame_lash_view_bounds: Option<crate::boneyard::BoneyardBounds>,
    pub player_light_registration: Option<crate::painter::PainterRegistration>,
    pub view_scale: f64,
    pub owner_id: &'a str,
    pub world_key: &'a str,
    pub targets: &'a [PrimarySpellTarget],
    pub can_place: &'a dyn Fn(&str, Vec2, f64) -> bool,
    pub can_traverse: &'a dyn Fn(PrimarySpellTraversalActor<'_>, Vec2, Vec2, f64, u32) -> bool,
    pub obstruction: &'a PrimarySpellObstruction<'a>,
    pub range_endpoint: &'a dyn Fn(Vec2, Vec2, f64) -> Vec2,
    pub navigation: Option<&'a dyn EnemyNavigation>,
}

/// Global old-actor queries, matching the source callbacks rather than borrowing
/// the first participant's region or homing targets. Departed owners still query
/// this boundary: Hub placement rejects them; Boneyard actors retain world access.
pub struct PrimarySpellEpochContext<'a> {
    pub tick: u64,
    pub enhanced_effects: bool,
    pub targets_for_owner: &'a dyn Fn(&str) -> &'a [PrimarySpellTarget],
    pub can_place: &'a dyn Fn(&str, Vec2, f64) -> bool,
    pub can_traverse: &'a dyn Fn(PrimarySpellTraversalActor<'_>, Vec2, Vec2, f64, u32) -> bool,
    pub obstruction: &'a PrimarySpellOwnerObstruction<'a>,
    pub navigation: Option<&'a dyn EnemyNavigation>,
}

pub type PrimarySpellOwnerObstruction<'a> =
    dyn Fn(&str, Vec2, Vec2, Option<&str>, u32) -> Option<Vec2> + 'a;

pub struct PrimarySpellPlayer<'a> {
    pub wizard: &'a mut HubWizard,
    /// College admission actually omits authority, preserving the old selection.
    pub authority: Option<PrimarySpellCastAuthority<'a>>,
    pub input: PrimarySpellInput,
    pub context: PrimarySpellWorldContext<'a>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrimarySpellPlayerResult {
    pub owner_id: String,
    pub mana_spent: f64,
    pub mana_underflow: bool,
}

#[derive(Default)]
pub struct PrimarySpellBatchTickResult {
    /// Shared contacts/emissions in source order. Mana fields sum the actual
    /// player debits; callers apply each participant's debit from `players`.
    pub tick: PrimarySpellTickResult,
    pub players: Vec<PrimarySpellPlayerResult>,
}

#[derive(Default)]
struct ManaDebit {
    remaining: f64,
    spent: f64,
    underflow: bool,
}

impl ManaDebit {
    // Native still casts with the available remainder. Reaching exactly zero also
    // selects the underpowered payload, even when the full cost was available.
    fn debit(&mut self, cost: f64) -> bool {
        self.underflow |= cost > self.remaining;
        let spent = self.remaining.max(0.0).min(cost);
        self.remaining = (self.remaining - spent).max(0.0);
        self.spent += spent;
        self.remaining <= 0.0
    }

    fn spend_exact(&mut self, cost: f64) -> bool {
        if self.remaining < cost {
            self.underflow = true;
            return false;
        }
        self.remaining -= cost;
        self.spent += cost;
        true
    }
}

impl SpellState {
    /// The single-participant API shares the same actor epoch and cast path.
    pub fn step_primary(
        &mut self,
        wizard: &mut HubWizard,
        authority: PrimarySpellCastAuthority<'_>,
        input: PrimarySpellInput,
        context: &PrimarySpellWorldContext<'_>,
        rng: &mut NativeRng,
        painter_order: Option<&mut WorldPainterOrder>,
    ) -> PrimarySpellTickResult {
        self.step_primary_one(wizard, Some(authority), input, context, rng, painter_order)
    }

    pub fn step_primary_without_authority(
        &mut self,
        wizard: &mut HubWizard,
        input: PrimarySpellInput,
        context: &PrimarySpellWorldContext<'_>,
        rng: &mut NativeRng,
        painter_order: &mut WorldPainterOrder,
    ) -> PrimarySpellTickResult {
        self.step_primary_one(wizard, None, input, context, rng, Some(painter_order))
    }

    fn step_primary_one(
        &mut self,
        wizard: &mut HubWizard,
        authority: Option<PrimarySpellCastAuthority<'_>>,
        input: PrimarySpellInput,
        context: &PrimarySpellWorldContext<'_>,
        rng: &mut NativeRng,
        painter_order: Option<&mut WorldPainterOrder>,
    ) -> PrimarySpellTickResult {
        let targets = |_: &str| context.targets;
        let obstruction = |_: &str, start, end, excluded: Option<&str>, mask| {
            (context.obstruction)(start, end, excluded, mask)
        };
        let epoch = PrimarySpellEpochContext {
            tick: context.tick,
            enhanced_effects: context.enhanced_effects,
            targets_for_owner: &targets,
            can_place: context.can_place,
            can_traverse: context.can_traverse,
            obstruction: &obstruction,
            navigation: context.navigation,
        };
        let mut players = [PrimarySpellPlayer { wizard, authority, input, context: *context }];
        self.step_primary_players(&mut players, &epoch, rng, painter_order).tick
    }

    /// Source `stepPrimarySpells`: advance the old queue once, cast in participant
    /// insertion order, then finalize old called rocks and new painter owners once.
    pub fn step_primary_players(
        &mut self,
        players: &mut [PrimarySpellPlayer<'_>],
        epoch: &PrimarySpellEpochContext<'_>,
        rng: &mut NativeRng,
        painter_order: Option<&mut WorldPainterOrder>,
    ) -> PrimarySpellBatchTickResult {
        let mut standalone_order = standalone_painter_order(self);
        let order = painter_order.unwrap_or(&mut standalone_order);
        let existing_called_rocks: Vec<u64> = self
            .transients
            .iter()
            .filter_map(|effect| {
                if let PrimaryTransientRef::EarthCalledRock(rock) = effect {
                    Some(rock.common.id)
                } else {
                    None
                }
            })
            .collect();
        let mut result = PrimarySpellBatchTickResult::default();
        self.advance_transients(epoch, rng, order, &mut result.tick);
        self.advance_projectiles(epoch, rng, order, &mut result.tick);
        for projectile in self.projectiles.iter() {
            if matches!(projectile, PrimaryProjectileRef::Fire(_))
                || matches!(projectile, PrimaryProjectileRef::Weld(spell) if spell.build_id==1000)
            {
                self.transients.push(PrimarySpellTransient::FireParticle(create_fire_particle(
                    self.next_id,
                    projectile.common(),
                    order,
                    epoch.enhanced_effects,
                )));
                self.next_id += 1;
            }
        }
        for player in players {
            assert_eq!(player.context.tick, epoch.tick, "one primary epoch has one tick");
            let cast = if let Some(authority) = player.authority {
                self.cast_primary_player(
                    player.wizard,
                    authority,
                    player.input,
                    &player.context,
                    rng,
                    order,
                )
            } else {
                self.advance_player_without_authority(player.wizard, player.input, &player.context);
                PrimarySpellTickResult::default()
            };
            result.players.push(PrimarySpellPlayerResult {
                owner_id: player.context.owner_id.into(),
                mana_spent: cast.mana_spent,
                mana_underflow: cast.mana_underflow,
            });
            result.tick.mana_spent += cast.mana_spent;
            result.tick.mana_underflow |= cast.mana_underflow;
            result.tick.frost_missile_world_contacts.extend(cast.frost_missile_world_contacts);
            result.tick.channel_emissions.extend(cast.channel_emissions);
            result.tick.fire_actor_contacts.extend(cast.fire_actor_contacts);
        }
        self.advance_called_rocks(&existing_called_rocks);
        self.enroll_weld_painters(order);
        result
    }

    fn advance_player_without_authority(
        &mut self,
        wizard: &mut HubWizard,
        input: PrimarySpellInput,
        context: &PrimarySpellWorldContext<'_>,
    ) {
        let previous = wizard.primary_cast.clone();
        let raw_held = input.primary && input.aim.is_some();
        let aim = if raw_held {
            primary_spell_aim_direction(wizard.position, input.aim.unwrap(), context.view_scale)
        } else {
            previous.aim_direction
        };
        wizard.primary_cast.advance(raw_held, false, WizardElement::Fire, 1.0);
        wizard.primary_cast.one_shot_attack_pose_held = false;
        wizard.primary_cast.selected_primary_age_ticks = previous.selected_primary_age_ticks + 1;
        wizard.primary_cast.weapon_pulse =
            fround(previous.weapon_pulse * NATIVE_PLAYER_WEAPON_PULSE_DECAY);
        if wizard.primary_cast.owns_facing() {
            wizard.set_facing(heading_degrees_from_vector(aim.x, aim.y));
        }
        wizard.primary_cast.aim_direction = aim;
        if previous.channel_active {
            self.transients.retain(|effect| {
                !matches!(effect, PrimaryTransientRef::Weld(WeldWorldRef::Persistent(actor))
                    if actor.common().owner.owner_id == context.owner_id)
            });
        }
        wizard.primary_cast.action_tick = None;
        wizard.primary_cast.channel_active = false;
        wizard.primary_cast.underpowered = false;
    }

    fn cast_primary_player(
        &mut self,
        wizard: &mut HubWizard,
        authority: PrimarySpellCastAuthority<'_>,
        input: PrimarySpellInput,
        context: &PrimarySpellWorldContext<'_>,
        rng: &mut NativeRng,
        order: &mut WorldPainterOrder,
    ) -> PrimarySpellTickResult {
        let element = match authority.primary_skill {
            NativePrimarySkillProfile::Air(_) => WizardElement::Air,
            NativePrimarySkillProfile::Earth(_) => WizardElement::Earth,
            NativePrimarySkillProfile::Ether(_) => WizardElement::Ether,
            NativePrimarySkillProfile::Fire(_) => WizardElement::Fire,
            NativePrimarySkillProfile::Water(_) => WizardElement::Water,
            // Native welded casts use the Fire action clock and weapon emitter.
            NativePrimarySkillProfile::Weld(_) => WizardElement::Fire,
        };
        let mut result = PrimarySpellTickResult::default();
        let previous = wizard.primary_cast.clone();
        let one_shot = match authority.primary_skill {
            NativePrimarySkillProfile::Weld(profile) => {
                profile.cast_kind == NativeWeldCastKind::OneShot
            }
            _ => matches!(element, WizardElement::Ether | WizardElement::Fire),
        };
        let mana_cost =
            authority.primary_skill.common().mana_cost / if one_shot { 1.0 } else { 100.0 };
        let mut mana = ManaDebit { remaining: authority.available_mana, ..ManaDebit::default() };
        let raw_held = input.primary && input.aim.is_some();
        let pressed = raw_held && !previous.held;
        let released = !raw_held && previous.held;
        let action_available = previous.action_tick.is_none()
            || (one_shot
                && !previous.channel_active
                && previous
                    .action_tick
                    .is_some_and(|tick| tick > primary_cast_completion_progress(element)));
        let accepted = raw_held
            && action_available
            && (pressed || (one_shot && previous.cast_sequence > 0))
            && authority.eligible;
        let aim = if raw_held {
            primary_spell_aim_direction(wizard.position, input.aim.unwrap(), context.view_scale)
        } else {
            previous.aim_direction
        };
        wizard.primary_cast.advance(raw_held, accepted, element, authority.cast_progress_factor);
        if !one_shot {
            wizard.primary_cast.one_shot_attack_pose_held = false;
        }
        let native_selected_primary_age = (previous.selected_primary_id
            == Some(authority.primary_skill.common().skill_id))
        .then_some(previous.selected_primary_age_ticks);
        wizard.primary_cast.selected_primary_age_ticks =
            native_selected_primary_age.map_or(0, |age| age + 1);
        wizard.primary_cast.selected_primary_id = Some(authority.primary_skill.common().skill_id);
        wizard.primary_cast.weapon_pulse =
            fround(previous.weapon_pulse * NATIVE_PLAYER_WEAPON_PULSE_DECAY);
        if wizard.primary_cast.owns_facing() {
            wizard.set_facing(heading_degrees_from_vector(aim.x, aim.y));
        }
        wizard.primary_cast.aim_direction = aim;

        if !authority.eligible {
            if element == WizardElement::Earth {
                self.projectiles.retain(|spell| {
                    !(matches!(spell, PrimaryProjectileRef::Earth(_))
                        && spell.common().owner_id == context.owner_id
                        && spell.common().phase == PrimarySpellProjectilePhase::Held)
                });
            }
            if previous.channel_active {
                self.transients.retain(|effect| {
                    !matches!(effect,
                    PrimaryTransientRef::Weld(WeldWorldRef::Persistent(actor))
                    if actor.common().owner.owner_id==context.owner_id)
                });
            }
            wizard.primary_cast.action_tick = None;
            wizard.primary_cast.channel_active = false;
            wizard.primary_cast.one_shot_attack_pose_held = false;
            wizard.primary_cast.underpowered = false;
            finish_ether_blast_tick(wizard, authority, mana.remaining, mana_cost);
            return result;
        }

        let mut earth_accepted_underpowered = false;
        if accepted {
            wizard.primary_cast.last_weld_playback_rate = None;
            wizard.primary_cast.last_weld_sound_variant = None;
            match authority.primary_skill {
                NativePrimarySkillProfile::Air(_) | NativePrimarySkillProfile::Water(_) => {
                    wizard.primary_cast.channel_active = true
                }
                NativePrimarySkillProfile::Earth(profile) => {
                    earth_accepted_underpowered = mana.debit(mana_cost);
                    let surged = profile.rock_surge_chance > 0.0
                        && f64::from(rng.draw_integer(10_000)) < profile.rock_surge_chance * 100.0
                        && mana.spend_exact(profile.rock_surge_mana_cost);
                    wizard.primary_cast.action_tick =
                        if surged { None } else { wizard.primary_cast.action_tick };
                    wizard.primary_cast.channel_active = !surged;
                    if surged {
                        wizard.primary_cast.emission_sequence += 1;
                    }
                    self.create_earth_boulder(
                        wizard,
                        profile,
                        authority.weapon_kind,
                        context,
                        surged,
                        order,
                    );
                }
                NativePrimarySkillProfile::Ether(_) | NativePrimarySkillProfile::Fire(_) => {}
                NativePrimarySkillProfile::Weld(profile) => {
                    if profile.cast_kind != NativeWeldCastKind::OneShot {
                        wizard.primary_cast.channel_active = true;
                    }
                }
            }
            if !one_shot {
                wizard.primary_cast.weapon_pulse = 0.25;
            }
        }

        let emission_progress = primary_cast_emission_progress(element);
        if one_shot
            && wizard.primary_cast.action_tick.is_some_and(|tick| tick >= emission_progress)
            && previous.action_tick.unwrap_or(-1.0) < emission_progress
        {
            if element == WizardElement::Ether && authority.primary_skill.common().skill_id == 8 {
                let charges =
                    native_ether_blast_release_charges(wizard.primary_cast.ether_blast_charge);
                wizard.primary_cast.ether_blast_charge = 0.0;
                if charges > 0 {
                    let presentation_rng = rng.clone();
                    let _particles = create_native_ether_blast_particle_program(rng);
                    let mut common =
                        transient_base(self.next_id, context.owner_id, context.world_key, None);
                    common.painter_registrations =
                        order.register_many(WorldManagerLane::Transient, 108);
                    self.transients.push(PrimarySpellTransient::EtherBlast(
                        PrimarySpellEtherBlast {
                            common,
                            birth_tick: context.tick,
                            charges,
                            origin: native_ether_blast_pulse_origin(wizard.position, aim),
                            presentation_rng,
                        },
                    ));
                    self.next_id += 1;
                }
            }
            let underpowered = mana.debit(mana_cost);
            let births = create_one_shot_projectiles(
                OneShotBirth {
                    first_id: self.next_id,
                    owner_id: context.owner_id,
                    wizard,
                    weapon: authority.weapon_kind,
                    profile: authority.primary_skill,
                    world_key: context.world_key,
                    targets: context.targets,
                    underpowered,
                },
                rng,
                order,
            );
            self.next_id += births.len() as u64;
            let weld_audio = births.first().and_then(|projectile| {
                if let PrimarySpellProjectile::Weld(spell) = projectile {
                    Some((spell.cast_playback_rate, spell.cast_sound_variant))
                } else {
                    None
                }
            });
            for born in births {
                let birth_mask = native_primary_birth_terrain_exclusion_mask(&born);
                let flight_mask = native_primary_flight_terrain_exclusion_mask(
                    PrimarySpellTraversalActor::Projectile(born.view()),
                );
                let common = born.common();
                let lookahead = Vec2::new(
                    common.position.x + common.velocity.x * 5.0,
                    common.position.y + common.velocity.y * 5.0,
                );
                let initial_clear = matches!(&born, PrimarySpellProjectile::Weld(spell) if spell.build_id==1009)
                    || (context.can_traverse)(
                        PrimarySpellTraversalActor::Projectile(born.view()),
                        wizard.position,
                        common.position,
                        0.0,
                        birth_mask,
                    );
                let clear = initial_clear
                    && (context.can_traverse)(
                        PrimarySpellTraversalActor::Projectile(born.view()),
                        common.position,
                        lookahead,
                        0.0,
                        flight_mask,
                    );
                if clear {
                    let mut spell = born;
                    self.advance_projectile_with_presentation(
                        &mut spell.view_mut(),
                        context.targets,
                        context.tick,
                        rng,
                    );
                    if matches!(&spell, PrimarySpellProjectile::Fire(_))
                        || matches!(&spell, PrimarySpellProjectile::Weld(weld) if weld.build_id==1000)
                    {
                        self.transients.push(PrimarySpellTransient::FireParticle(
                            create_fire_particle(
                                self.next_id,
                                spell.common(),
                                order,
                                context.enhanced_effects,
                            ),
                        ));
                        self.next_id += 1;
                    }
                    self.projectiles.push(spell);
                } else {
                    self.projectile_terrain_impact(
                        &born,
                        context.tick,
                        context.enhanced_effects,
                        rng,
                        order,
                        &mut result,
                    );
                }
            }
            wizard.primary_cast.emission_sequence += 1;
            wizard.primary_cast.last_weld_playback_rate = weld_audio.map(|audio| audio.0);
            wizard.primary_cast.last_weld_sound_variant = weld_audio.and_then(|audio| audio.1);
            if underpowered {
                wizard.primary_cast.fizzle_sequence += 1;
            }
            wizard.primary_cast.one_shot_attack_pose_held = true;
            wizard.primary_cast.underpowered = underpowered;
            wizard.primary_cast.weapon_pulse = 0.15_f32 as f64;
        }

        let earth_release_eligible = element == WizardElement::Earth
            && self.projectiles.iter().any(|spell| {
                matches!(spell, PrimaryProjectileRef::Earth(_))
                    && spell.common().owner_id == context.owner_id
                    && spell.common().phase == PrimarySpellProjectilePhase::Held
                    && spell.common().charge >= 0.3_f32 as f64
            });
        if wizard.primary_cast.channel_active {
            match authority.primary_skill {
                NativePrimarySkillProfile::Weld(_) if raw_held => {
                    let underpowered = mana.debit(mana_cost);
                    self.step_weld_cast(
                        wizard,
                        authority,
                        underpowered,
                        native_selected_primary_age,
                        context,
                        rng,
                        order,
                        &mut result,
                    );
                }
                NativePrimarySkillProfile::Air(_) if raw_held => {
                    let underpowered = mana.debit(mana_cost);
                    let origin = emitter(wizard, element, authority.weapon_kind);
                    let (endpoint, midpoint, target_id) =
                        air_geometry(wizard, origin, aim, context);
                    result.channel_emissions.push(channel_emission(
                        self.next_id,
                        authority.primary_skill,
                        origin,
                        Some(endpoint),
                        wizard.position,
                        aim,
                        underpowered,
                        mana_cost,
                        context,
                    ));
                    let mut common =
                        transient_base(self.next_id, context.owner_id, context.world_key, None);
                    common.painter_registrations = order.register_many(WorldManagerLane::Actor, 3);
                    common.light_registration = Some(order.register(WorldManagerLane::Transient));
                    self.transients.push(PrimarySpellTransient::Air(PrimarySpellAirTransient {
                        common,
                        chained: false,
                        enhanced_effects: context.enhanced_effects,
                        birth_tick: context.tick,
                        direction: aim,
                        endpoint,
                        hurricane_charge: 0.0,
                        midpoint,
                        origin,
                        target_id: target_id.clone(),
                        underpowered,
                        variant: (self.next_id % 4) as u32,
                    }));
                    wizard.primary_cast.target_id = target_id;
                    wizard.primary_cast.underpowered = underpowered;
                    self.next_id += 1;
                }
                NativePrimarySkillProfile::Water(profile) if raw_held => {
                    let underpowered = mana.debit(mana_cost);
                    let origin = emitter(wizard, element, authority.weapon_kind);
                    result.channel_emissions.push(channel_emission(
                        self.next_id,
                        authority.primary_skill,
                        origin,
                        None,
                        wizard.position,
                        aim,
                        underpowered,
                        mana_cost,
                        context,
                    ));
                    let widen = if underpowered { 0.0 } else { profile.widen_half_degrees };
                    let count = if underpowered {
                        1
                    } else {
                        water_frost_jet_particle_count(widen, context.enhanced_effects)
                    };
                    let speed = water_frost_jet_speed(widen);
                    for variant in 0..count {
                        let id = self.next_id + u64::from(variant);
                        let born = water_frost_jet_emission(WaterFrostJetEmissionInput {
                            emitter: origin,
                            base_direction: aim,
                            tick: context.tick,
                            ordinal: variant,
                            id,
                            particle_count: count,
                            speed,
                            widen_half_degrees: widen,
                        });
                        let obstruction = water_frost_jet_obstruction(
                            born,
                            wizard.position,
                            id,
                            &|start, end| {
                                (context.obstruction)(
                                    start,
                                    end,
                                    None,
                                    NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK,
                                )
                            },
                            underpowered,
                        );
                        let mut common =
                            transient_base(id, context.owner_id, context.world_key, None);
                        common.age_ticks = 1;
                        common.painter_registrations =
                            order.register_many(WorldManagerLane::Transient, 1);
                        self.transients.push(PrimarySpellTransient::Water(
                            PrimarySpellWaterTransient {
                                common,
                                direction: born.direction,
                                obstruction_distance: obstruction.map(|hit| hit.distance),
                                obstruction_point: obstruction.map(|hit| hit.point),
                                origin: born.origin,
                                speed: born.speed,
                                underpowered,
                                variant,
                            },
                        ));
                    }
                    self.next_id += u64::from(count);
                    wizard.primary_cast.underpowered = underpowered;
                }
                NativePrimarySkillProfile::Earth(profile) => {
                    let underpowered =
                        if accepted { earth_accepted_underpowered } else { mana.debit(mana_cost) };
                    let origin = emitter(wizard, element, authority.weapon_kind);
                    for mut projectile in self.projectiles.iter_mut() {
                        let PrimaryProjectilePartsMut::Earth(mut spell) = projectile.view_mut()
                        else {
                            continue;
                        };
                        if spell.common.owner_id != context.owner_id
                            || spell.common.phase != PrimarySpellProjectilePhase::Held
                        {
                            continue;
                        }
                        let previous_charge = spell.common.charge;
                        let release_threshold = 0.3_f32 as f64;
                        let charge = if accepted
                            || (!raw_held && previous_charge >= release_threshold)
                            || (underpowered && previous_charge > release_threshold)
                        {
                            previous_charge
                        } else {
                            advance_native_earth_boulder_charge(
                                previous_charge,
                                profile.growth_factor,
                                spell.maximum_charge,
                                1.0,
                            )
                        };
                        if (30.0 * previous_charge).floor() != (30.0 * charge).floor() {
                            spell.assembly_charge = charge;
                        }
                        if underpowered && previous_charge < 1.0 {
                            spell.common.damage = fround(spell.common.damage * 0.5);
                            spell.remaining_damage = fround(spell.remaining_damage * 0.5);
                        }
                        spell.common.charge = charge;
                        spell.common.direction = aim;
                        if raw_held || previous_charge < release_threshold {
                            spell.orientation =
                                earth_boulder_held_orientation_step(spell.orientation, aim);
                        }
                        spell.common.position = Vec2::new(origin.x, origin.y + 15.0);
                        spell.shell_charge = spell.assembly_charge;
                        spell.common.world_key = context.world_key.into();
                    }
                    if underpowered && context.tick.is_multiple_of(50) {
                        wizard.primary_cast.fizzle_sequence += 1;
                    }
                    wizard.primary_cast.underpowered = underpowered;
                    let held_boulder = self.projectiles.iter().find_map(|projectile| {
                        if let PrimaryProjectileRef::Earth(spell) = projectile
                            && spell.common.owner_id == context.owner_id
                            && spell.common.phase == PrimarySpellProjectilePhase::Held
                        {
                            Some(spell)
                        } else {
                            None
                        }
                    });
                    if let Some(boulder) = held_boulder
                        && (raw_held || !earth_release_eligible)
                        && boulder.common.charge < boulder.maximum_charge
                        && earth_called_rock_emits(boulder, context.tick)
                    {
                        self.transients.push(PrimarySpellTransient::EarthCalledRock(
                            create_earth_called_rock(
                                self.next_id,
                                boulder,
                                order.register(WorldManagerLane::Actor),
                            ),
                        ));
                        self.next_id += 1;
                    }
                }
                _ => {}
            }
        }
        let end_channel = wizard.primary_cast.channel_active
            && if element == WizardElement::Earth {
                !raw_held && earth_release_eligible
            } else {
                released
            };
        if end_channel {
            if element == WizardElement::Earth {
                self.release_earth_boulders(aim, context, rng, order, &mut result);
                wizard.primary_cast.emission_sequence += 1;
            }
            wizard.primary_cast.action_tick = None;
            wizard.primary_cast.channel_active = false;
            wizard.primary_cast.target_id = None;
            wizard.primary_cast.underpowered = false;
            if matches!(authority.primary_skill, NativePrimarySkillProfile::Weld(_)) {
                self.release_owned_weld_persistent_actors(
                    context.owner_id,
                    context.tick,
                    rng,
                    order,
                );
            }
        }
        finish_ether_blast_tick(wizard, authority, mana.remaining, mana_cost);
        result.mana_spent = mana.spent;
        result.mana_underflow = mana.underflow;
        result
    }

    fn create_earth_boulder(
        &mut self,
        wizard: &HubWizard,
        profile: &NativeEarthPrimarySkillProfile,
        weapon: Option<NativePlayerWeaponKind>,
        context: &PrimarySpellWorldContext<'_>,
        surged: bool,
        order: &mut WorldPainterOrder,
    ) {
        let aim = wizard.primary_cast.aim_direction;
        let initial = 0.18_f32 as f64;
        let charge = if surged {
            profile.maximum_charge
        } else {
            advance_native_earth_boulder_charge(
                initial,
                profile.growth_factor,
                profile.maximum_charge,
                1.0,
            )
        };
        let origin = emitter(wizard, WizardElement::Earth, weapon);
        let position = if surged {
            Vec2::new(
                fround(wizard.position.x + aim.x * 60.0),
                fround(wizard.position.y + aim.y * 60.0),
            )
        } else {
            Vec2::new(origin.x, origin.y + 15.0)
        };
        let velocity =
            if surged { Vec2::new(fround(aim.x * 3.0), fround(aim.y * 3.0)) } else { Vec2::ZERO };
        let common = PrimarySpellProjectileBase {
            age_ticks: 1,
            charge,
            damage: profile.common.damage_minimum,
            direction: aim,
            flight_ticks: u64::from(surged),
            id: self.next_id,
            registration: order.register(WorldManagerLane::Actor),
            owner_id: context.owner_id.into(),
            phase: if surged {
                PrimarySpellProjectilePhase::Flight
            } else {
                PrimarySpellProjectilePhase::Held
            },
            position,
            velocity,
            world_key: context.world_key.into(),
        };
        self.projectiles.push(PrimarySpellProjectile::Earth(PrimarySpellEarthProjectile {
            common,
            program: EarthProjectileProgram {
                assembly_charge: if surged { charge } else { initial },
                hit_target_ids: Vec::new(),
                maximum_charge: profile.maximum_charge,
                orientation: if surged {
                    earth_boulder_flight_orientation_step(
                        EARTH_BOULDER_IDENTITY_ORIENTATION,
                        aim,
                        velocity,
                        charge,
                    )
                } else {
                    EARTH_BOULDER_IDENTITY_ORIENTATION
                },
                remaining_damage: if surged {
                    native_earth_boulder_released_damage(profile.common.damage_minimum, charge, 1.0)
                } else {
                    profile.common.damage_minimum
                },
                shell_charge: if surged { charge } else { initial },
                toughness: profile.toughness,
            },
        }));
        self.next_id += 1;
    }

    fn advance_projectiles(
        &mut self,
        context: &PrimarySpellEpochContext<'_>,
        rng: &mut NativeRng,
        order: &mut WorldPainterOrder,
        result: &mut PrimarySpellTickResult,
    ) {
        let mut projectiles = std::mem::take(&mut self.projectiles);
        projectiles.retain_mut(|stored| {
            let mut spell = stored.view_mut();
            if spell.common().phase == PrimarySpellProjectilePhase::Held {
                advance_projectile(&mut spell, &[]);
                return true;
            }
            let common = spell.common();
            if !matches!(spell, PrimaryProjectilePartsMut::Earth(_))
                && common.age_ticks.is_multiple_of(5)
                && !(context.can_traverse)(
                    PrimarySpellTraversalActor::Projectile(spell.view()),
                    common.position,
                    Vec2::new(
                        common.position.x + common.velocity.x * 5.0,
                        common.position.y + common.velocity.y * 5.0,
                    ),
                    0.0,
                    native_primary_flight_terrain_exclusion_mask(
                        PrimarySpellTraversalActor::Projectile(spell.view()),
                    ),
                )
            {
                self.projectile_terrain_impact(
                    &spell.snapshot(),
                    context.tick,
                    context.enhanced_effects,
                    rng,
                    order,
                    result,
                );
                return false;
            }
            let targets = (context.targets_for_owner)(&spell.common().owner_id);
            self.advance_projectile_with_presentation(&mut spell, targets, context.tick, rng);
            let common = spell.common();
            if matches!(spell, PrimaryProjectilePartsMut::Earth(_))
                && !(context.can_traverse)(
                    PrimarySpellTraversalActor::Projectile(spell.view()),
                    common.position,
                    Vec2::new(
                        fround(common.position.x + common.velocity.x),
                        fround(common.position.y + common.velocity.y),
                    ),
                    common.charge * 75.0,
                    0,
                )
            {
                self.projectile_terrain_impact(
                    &spell.snapshot(),
                    context.tick,
                    context.enhanced_effects,
                    rng,
                    order,
                    result,
                );
                false
            } else {
                true
            }
        });
        self.projectiles = projectiles;
    }

    fn projectile_terrain_impact(
        &mut self,
        spell: &PrimarySpellProjectile,
        tick: u64,
        enhanced_effects: bool,
        rng: &mut NativeRng,
        order: &mut WorldPainterOrder,
        result: &mut PrimarySpellTickResult,
    ) {
        match spell {
            PrimarySpellProjectile::Weld(spell) => {
                let birth = NativeWeldActorBirth {
                    build_id: spell.build_id,
                    direction: spell.common.direction,
                    id: self.next_id,
                    origin: spell.common.position,
                    owner_id: &spell.common.owner_id,
                    tick,
                    vector: &spell.vector,
                    world_key: &spell.common.world_key,
                };
                if spell.build_id == 1000 {
                    let detonation = create_primary_spell_weld_fire_detonation(
                        birth,
                        rng,
                        spell.presentation_seed.unwrap_or(0),
                        true,
                        order,
                    );
                    self.next_id = detonation.next_id;
                    self.transients.extend(detonation.transients);
                    result
                        .fire_actor_contacts
                        .extend(detonation.contacts.into_iter().map(FireActorContact::Ember));
                } else {
                    if spell.build_id == 1001 {
                        result.frost_missile_world_contacts.push(spell.clone());
                    }
                    self.transients.push(PrimarySpellTransient::Weld(
                        NativeWeldWorldActor::Impact(create_primary_spell_weld_impact(birth, rng)),
                    ));
                    self.next_id += 1;
                }
            }
            PrimarySpellProjectile::Fire(spell) => {
                let detonation = create_primary_spell_fire_detonation(
                    self.next_id,
                    spell,
                    spell.common.position,
                    rng,
                    order,
                );
                self.next_id = detonation.next_id;
                self.transients.extend(detonation.transients);
                result
                    .fire_actor_contacts
                    .extend(detonation.contacts.into_iter().map(FireActorContact::Ember));
            }
            PrimarySpellProjectile::Ether(spell) => {
                self.transients.push(PrimarySpellTransient::EtherImpact(
                    create_primary_spell_ether_impact(self.next_id, spell, tick, order),
                ));
                self.next_id += 1;
            }
            PrimarySpellProjectile::Earth(spell) => {
                self.transients.push(PrimarySpellTransient::EarthImpact(
                    create_primary_spell_earth_impact(
                        self.next_id,
                        spell,
                        tick,
                        order,
                        enhanced_effects,
                    ),
                ));
                self.next_id += 1;
            }
        }
    }

    fn release_earth_boulders(
        &mut self,
        aim: Vec2,
        context: &PrimarySpellWorldContext<'_>,
        rng: &mut NativeRng,
        order: &mut WorldPainterOrder,
        result: &mut PrimarySpellTickResult,
    ) {
        let mut projectiles = std::mem::take(&mut self.projectiles);
        projectiles.retain_mut(|projectile| {
            let PrimaryProjectilePartsMut::Earth(mut spell) = projectile.view_mut() else {
                return true;
            };
            if spell.common.owner_id != context.owner_id
                || spell.common.phase != PrimarySpellProjectilePhase::Held
            {
                return true;
            }
            let velocity = Vec2::new(fround(aim.x * 3.0), fround(aim.y * 3.0));
            let position = Vec2::new(
                fround(spell.common.position.x + velocity.x),
                fround(spell.common.position.y + velocity.y),
            );
            let stored_delta = Vec2::new(
                fround(position.x - spell.common.position.x),
                fround(position.y - spell.common.position.y),
            );
            spell.common.direction = aim;
            spell.common.flight_ticks = 1;
            spell.orientation = earth_boulder_flight_orientation_step(
                spell.orientation,
                aim,
                stored_delta,
                spell.common.charge,
            );
            spell.maximum_charge = spell.common.charge;
            spell.common.phase = PrimarySpellProjectilePhase::Flight;
            spell.common.position = position;
            spell.remaining_damage = native_earth_boulder_released_damage(
                spell.remaining_damage,
                spell.common.charge,
                1.0,
            );
            spell.common.velocity = velocity;
            let next_position =
                Vec2::new(fround(position.x + velocity.x), fround(position.y + velocity.y));
            let radius = spell.common.charge * 75.0;
            if (context.can_traverse)(
                PrimarySpellTraversalActor::Projectile(projectile.view()),
                position,
                next_position,
                radius,
                0,
            ) {
                true
            } else {
                self.projectile_terrain_impact(
                    &projectile.snapshot(),
                    context.tick,
                    context.enhanced_effects,
                    rng,
                    order,
                    result,
                );
                false
            }
        });
        self.projectiles = projectiles;
    }

    fn advance_called_rocks(&mut self, existing_ids: &[u64]) {
        self.transients.retain_mut(|effect| {
            let PrimaryTransientMut::EarthCalledRock(rock) = effect else {
                return true;
            };
            if !existing_ids.contains(&rock.common.id) {
                return true;
            }
            let parent = self.projectiles.iter().find_map(|spell| {
                if let PrimaryProjectileRef::Earth(boulder) = spell
                    && boulder.common.id == rock.parent_id
                {
                    Some(boulder)
                } else {
                    None
                }
            });
            advance_earth_called_rock(rock, parent)
        });
    }
}

fn finish_ether_blast_tick(
    wizard: &mut HubWizard,
    authority: PrimarySpellCastAuthority<'_>,
    available_mana: f64,
    mana_cost: f64,
) {
    if authority.plane_active {
        wizard.primary_cast.ether_blast_charge = 0.0;
        return;
    }
    let NativePrimarySkillProfile::Ether(profile) = authority.primary_skill else {
        return;
    };
    if !authority.alive || profile.common.skill_id != 8 {
        return;
    }
    let charge = advance_native_ether_blast_charge(
        wizard.primary_cast.ether_blast_charge,
        profile.blast_charge_capacity,
        available_mana >= mana_cost,
        false,
    );
    wizard.primary_cast.ether_blast_charge = charge.charge;
    if charge.crossed_integer {
        wizard.primary_cast.ether_blast_charge_cue_sequence += 1;
        wizard.primary_cast.weapon_pulse = NATIVE_ETHER_BLAST_WEAPON_PULSE;
    }
}

pub(super) fn air_geometry(
    wizard: &HubWizard,
    origin: Vec2,
    aim: Vec2,
    context: &PrimarySpellWorldContext<'_>,
) -> (Vec2, Vec2, Option<String>) {
    let range_endpoint = (context.range_endpoint)(wizard.position, aim, 0.0);
    let mut endpoint = (context.obstruction)(
        wizard.position,
        range_endpoint,
        None,
        NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK,
    )
    .unwrap_or(range_endpoint);
    let has_line_of_sight = |target: &PrimarySpellTarget| {
        (context.obstruction)(
            wizard.position,
            target.position,
            Some(&target.id),
            NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK,
        )
        .is_none()
    };
    let target = select_air_primary_target(AirPrimaryTargetQuery {
        aim_direction: aim,
        has_line_of_sight: &has_line_of_sight,
        maximum_range: hypot(
            range_endpoint.x - wizard.position.x,
            range_endpoint.y - wizard.position.y,
        ),
        origin: wizard.position,
        previous_target_id: wizard.primary_cast.target_id.as_deref(),
        targets: context.targets,
    });
    if let Some(target) = target {
        let attachment = Vec2::new(
            target.position.x + target.attachment.x,
            target.position.y + target.attachment.y,
        );
        let clipped = (context.obstruction)(
            wizard.position,
            attachment,
            Some(&target.id),
            NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK,
        )
        .unwrap_or(attachment);
        endpoint = Vec2::new(clipped.x, clipped.y + AIR_PRIMARY_TARGET_Y_OFFSET);
    }
    let geometry = air_primary_bolt_geometry(origin, aim, endpoint);
    (geometry.endpoint, geometry.midpoint, target.map(|target| target.id.clone()))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn channel_emission(
    id: u64,
    profile: &NativePrimarySkillProfile,
    origin: Vec2,
    endpoint: Option<Vec2>,
    query_origin: Vec2,
    direction: Vec2,
    underpowered: bool,
    mana_cost: f64,
    context: &PrimarySpellWorldContext<'_>,
) -> PrimarySpellChannelEmission {
    PrimarySpellChannelEmission {
        flame_lash_presentation: None,
        damage: profile.common().damage_minimum / 100.0 * if underpowered { 0.5 } else { 1.0 },
        direction,
        endpoint,
        id,
        kind: match profile {
            NativePrimarySkillProfile::Air(_) => PrimarySpellChannelKind::Air,
            NativePrimarySkillProfile::Weld(_) => PrimarySpellChannelKind::Weld,
            _ => PrimarySpellChannelKind::Water,
        },
        mana_cost,
        origin,
        owner_id: context.owner_id.into(),
        primary_skill: profile.clone(),
        query_origin,
        terrain_contact: false,
        underpowered,
        world_key: context.world_key.into(),
    }
}
