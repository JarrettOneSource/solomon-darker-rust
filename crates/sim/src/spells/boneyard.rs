//! Primary phases composed from `core-server/game-simulation.ts`.
//!
//! The session steps world movement/enemies and secondary actors first. Cast then
//! commits, secondary damage/knockback resolves, and primary contacts resolve last.
//! This module assembles the source authority and callbacks around the canonical
//! kernels for one local player; it never owns another copy of player or enemy state.

use super::casting::NativePlayerWeaponKind;
use super::contacts::{
    BoneyardSpecialDeathResult, BoneyardSpellCombatContext, BoneyardSpellCombatResult,
    BoneyardSpellHitKind, resolve_boneyard_spell_combat,
};
use super::model::{PrimarySpellProjectile, PrimarySpellTickResult, SpellState};
use super::targeting::{
    PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
    native_primary_view_ray_endpoint,
};
use super::{
    PrimarySpellCastAuthority, PrimarySpellInput, PrimarySpellTraversalActor,
    PrimarySpellWorldContext,
};
use crate::boneyard::collision::{
    BoneyardCollisionWorld, can_place_boneyard_body, first_boneyard_line_obstruction,
    first_boneyard_path_block_progress, resolve_boneyard_movement,
};
use crate::boneyard::navigation::{NavigationSet, RouteRequest};
use crate::boneyard::world_light::native_primary_view_bounds;
use crate::boneyard::{
    BONEYARD_CAMERA_SCALE, BoneyardBounds, BoneyardWorld, native_boneyard_fence_post_positions,
};
use crate::combat::PlayerLifeState;
use crate::economy::EquipmentType;
use crate::enemies::model::{EnemyActor, EnemyLifeState, EnemyMaggotActor};
use crate::enemies::path::EnemyNavigation;
use crate::enemies::targeting::{boneyard_enemy_actor_flags, boneyard_enemy_collision_radius};
use crate::enemies::world::EnemyWorld;
use crate::float::hypot;
use crate::hub::HubWizard;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::player::PlayerState;
use crate::profile::native_hagatha_boss_damage_factor;
use crate::profile::primary::native_primary_skill_profile_with_stat_book;
use crate::secondary::{
    NativeSecondaryEtherBurnInput, NativeSecondaryFireBurnInput, NativeSecondarySimulationState,
    NativeSecondaryTarget, apply_native_secondary_target_effect,
    enroll_native_secondary_painter_owners, native_secondary_target_effect,
};
use crate::{NativeRng, Vec2};

/// Values sampled once by the session. Additional scenery is the live loot owner's
/// actual goodie rows; the authored scenery projection belongs to this module.
#[derive(Clone, Copy)]
pub struct BoneyardPrimaryFrame<'a> {
    pub enhanced_effects: bool,
    pub player_light_registration: Option<PainterRegistration>,
    pub owner_id: &'a str,
    pub tick: u64,
    pub input: PrimarySpellInput,
    pub viewport: Vec2,
    pub eligible: bool,
    /// Staff/secondary casts can suppress primary input in the preceding phases.
    pub primary_overridden: bool,
    pub navigation: &'a NavigationSet,
    pub additional_scenery_targets: &'a [PrimarySpellTarget],
    /// The real logical world-light sampler, including the incoming light camera.
    pub light_at: &'a dyn Fn(Vec2) -> f64,
}

pub struct BoneyardPrimaryContactInput<'a> {
    pub frame: BoneyardPrimaryFrame<'a>,
    pub cast: &'a PrimarySpellTickResult,
    pub secondary_deaths: Vec<BoneyardSpecialDeathResult>,
}

/// Shared late-contact inputs. Participant casts and geometry are already committed.
pub struct BoneyardPrimaryContactEpoch<'a> {
    pub enhanced_effects: bool,
    pub tick: u64,
    pub cast: &'a PrimarySpellTickResult,
    pub secondary_deaths: Vec<BoneyardSpecialDeathResult>,
    pub additional_scenery_targets: &'a [PrimarySpellTarget],
    pub light_at: &'a dyn Fn(Vec2) -> f64,
}

/// Cast/flight phase. Source secondary damage and knockback belong between this
/// call and `resolve_boneyard_primary_contacts`.
#[allow(clippy::too_many_arguments)] // Participant movement and shared world have separate owners.
pub fn step_boneyard_primary_cast(
    world: &BoneyardWorld,
    wizard: &mut HubWizard,
    player: &mut PlayerState,
    secondary: &NativeSecondarySimulationState,
    spells: &mut SpellState,
    frame: BoneyardPrimaryFrame<'_>,
    combat_rng: &mut NativeRng,
    painters: &mut WorldPainterOrder,
) -> PrimarySpellTickResult {
    let derived = player.derived_stats();
    let profile = native_primary_skill_profile_with_stat_book(
        &player.book,
        &player.stat_book,
        &derived.offensive_factors(&player.skill_runtime),
    );
    let weapon_kind =
        match player.economy.equipment.weapon.as_ref().and_then(|item| item.equipment_type) {
            Some(EquipmentType::Staff) => Some(NativePlayerWeaponKind::Staff),
            Some(EquipmentType::Wand) => Some(NativePlayerWeaponKind::Wand),
            _ => None,
        };
    let secondary_player = secondary.players.get(frame.owner_id);
    let secondary_casting = secondary_player
        .is_some_and(|state| state.cast_action.is_some() || state.cast_spin_ticks_remaining > 0);
    let admission_enabled =
        world.encounter.as_ref().is_none_or(|encounter| encounter.combat_enabled());
    let input = PrimarySpellInput {
        primary: frame.input.primary
            && admission_enabled
            && !frame.primary_overridden
            && !secondary_casting,
        ..frame.input
    };
    let authority = PrimarySpellCastAuthority {
        weapon_kind,
        alive: player.combat.life_state == PlayerLifeState::Alive,
        available_mana: player.combat.current_mana,
        cast_progress_factor: derived.cast_progress_factor,
        eligible: frame.eligible
            && player.combat.can_cast()
            && player.progression.pending_offer.is_none(),
        plane_active: secondary_player.is_some_and(|state| state.planewalker_ticks_remaining > 0),
        primary_skill: &profile,
    };
    let bounds = world.active_bounds();
    let camera_bounds = world.camera_bounds();
    let focus = wizard.position;
    let world_key = format!("boneyard:{}", world.loaded.run_id);
    let mut targets = boneyard_primary_scenery_targets(world);
    targets.extend_from_slice(frame.additional_scenery_targets);
    targets.extend(world.enemies.spell_targets());
    let gravestone_ids = world.scenery_gravestone_target_ids();
    let gravestones: Vec<_> = gravestone_ids.iter().map(String::as_str).collect();
    let geometry = BoneyardPrimaryGeometry {
        bounds,
        collision: &world.dynamic_collision,
        static_collision: &world.collision,
        prepared: frame.navigation,
        gravestones: &gravestones,
    };
    let can_place = |_: &str, position, radius| geometry.can_place(position, radius);
    let can_traverse = |spell: PrimarySpellTraversalActor<'_>, from, to, radius, mask| {
        geometry.can_traverse(spell, from, to, radius, mask)
    };
    let obstruction = |start, end, excluded_source_id: Option<&str>, native_mask| {
        geometry.obstruction(start, end, excluded_source_id, native_mask)
    };
    let range_endpoint = |start, direction, padding| {
        let view = native_primary_view_bounds(
            camera_bounds,
            focus,
            frame.viewport,
            BONEYARD_CAMERA_SCALE,
            padding,
        );
        native_primary_view_ray_endpoint(start, direction, view)
    };
    let result = spells.step_primary(
        wizard,
        authority,
        input,
        &PrimarySpellWorldContext {
            enhanced_effects: frame.enhanced_effects,
            flame_lash_view_bounds: Some(native_primary_view_bounds(
                camera_bounds,
                focus,
                frame.viewport,
                BONEYARD_CAMERA_SCALE,
                25.0,
            )),
            player_light_registration: frame.player_light_registration,
            tick: frame.tick,
            view_scale: BONEYARD_CAMERA_SCALE,
            owner_id: frame.owner_id,
            world_key: &world_key,
            targets: &targets,
            can_place: &can_place,
            can_traverse: &can_traverse,
            obstruction: &obstruction,
            range_endpoint: &range_endpoint,
            navigation: Some(&geometry),
        },
        combat_rng,
        Some(painters),
    );
    if result.mana_spent > 0.0 {
        assert!(
            player.try_debit_mana(result.mana_spent).accepted,
            "primary spell mana authority diverged"
        );
    }
    result
}

/// Late primary contact phase. Enemy semantic events stay in the enemy world;
/// returned notices let the session score kills before the later death/XP epoch.
pub fn resolve_boneyard_primary_contacts(
    world: &mut BoneyardWorld,
    player: &PlayerState,
    secondary: &mut NativeSecondarySimulationState,
    spells: &mut SpellState,
    input: BoneyardPrimaryContactInput<'_>,
    combat_rng: &mut NativeRng,
    painters: &mut WorldPainterOrder,
) -> BoneyardSpellCombatResult {
    resolve_boneyard_primary_contacts_for_players(
        world,
        &[BoneyardPrimaryContactPlayer {
            owner_id: input.frame.owner_id,
            player,
            viewport: input.frame.viewport,
        }],
        secondary,
        spells,
        BoneyardPrimaryContactEpoch {
            enhanced_effects: input.frame.enhanced_effects,
            tick: input.frame.tick,
            cast: input.cast,
            secondary_deaths: input.secondary_deaths,
            additional_scenery_targets: input.frame.additional_scenery_targets,
            light_at: input.frame.light_at,
        },
        combat_rng,
        painters,
    )
}

pub struct BoneyardPrimaryContactPlayer<'a> {
    pub owner_id: &'a str,
    pub player: &'a PlayerState,
    pub viewport: Vec2,
}

/// One global contact traversal resolves each retained effect's actual owner.
/// The one-player adapter above uses this same path.
pub fn resolve_boneyard_primary_contacts_for_players(
    world: &mut BoneyardWorld,
    players: &[BoneyardPrimaryContactPlayer<'_>],
    secondary: &mut NativeSecondarySimulationState,
    spells: &mut SpellState,
    input: BoneyardPrimaryContactEpoch<'_>,
    combat_rng: &mut NativeRng,
    painters: &mut WorldPainterOrder,
) -> BoneyardSpellCombatResult {
    let world_key = format!("boneyard:{}", world.loaded.run_id);
    let mut scenery = boneyard_primary_scenery_targets(world);
    scenery.extend_from_slice(input.additional_scenery_targets);
    let native_types: Vec<_> = world
        .enemies
        .actors
        .iter()
        .map(|actor| (actor.id, u32::from(actor.config.native_type_id)))
        .collect();
    // game-simulation.ts uses full Region bounds for contact LOS/pushback, while
    // cast flight above uses the newly active Arena bounds. Preserve that split.
    let bounds = world.loaded.scene.bounds;
    let collision = &world.dynamic_collision;
    let first_contact = |start, end, radius| {
        first_boneyard_path_block_progress(start, end, bounds, collision, radius, &[])
    };
    let mut resolve_movement = |_, start, requested, radius| {
        resolve_boneyard_movement(start, requested, bounds, collision, radius)
    };
    let damage_multiplier = |target_id, kind, owner_id: &str| {
        let prismatic = kind == BoneyardSpellHitKind::Air
            && native_secondary_target_effect(&secondary.target_effects, &world_key, target_id)
                .is_some_and(|effect| effect.prismatic_ticks > 0);
        let boss_factor =
            native_types.iter().find(|(id, _)| *id == target_id).map_or(1.0, |(_, type_id)| {
                players.iter().find(|row| row.owner_id == owner_id).map_or(1.0, |row| {
                    native_hagatha_boss_damage_factor(
                        &row.player.economy.owned_perk_selectors,
                        *type_id,
                    )
                })
            });
        if prismatic { 2.0 * boss_factor } else { boss_factor }
    };
    let result = resolve_boneyard_spell_combat(
        &mut world.enemies,
        spells,
        BoneyardSpellCombatContext {
            enhanced_effects: input.enhanced_effects,
            channel_emissions: &input.cast.channel_emissions,
            fire_actor_contacts: &input.cast.fire_actor_contacts,
            frost_missile_world_contacts: &input.cast.frost_missile_world_contacts,
            tick: input.tick,
            world_key: &world_key,
            first_world_contact: Some(&first_contact),
            damage_multiplier: &damage_multiplier,
            primary_scenery_targets: &scenery,
            resolve_enemy_movement: &mut resolve_movement,
            fireball_hostile_corridor_length: &|owner_id| {
                players
                    .iter()
                    .find(|row| row.owner_id == owner_id)
                    .map_or(1600.0, |row| row.viewport.x)
            },
            light_at: Some(input.light_at),
        },
        combat_rng,
        Some(painters),
    );

    let mut secondary_deaths = input.secondary_deaths;
    for death in &mut secondary_deaths {
        spells.transients.append(&mut death.spells.transients);
    }
    let previous_actor_id = secondary.next_actor_id;
    for burn in secondary_deaths.iter().flat_map(|death| &death.combat.burns).chain(&result.burns) {
        if let Some(owner) = players.iter().find(|row| row.owner_id == burn.owner_id)
            && let Some(target) = boneyard_native_secondary_target(&world.enemies, burn.target_id)
        {
            secondary.apply_fire_burn(NativeSecondaryFireBurnInput {
                damage: burn.damage,
                owner_id: burn.owner_id.clone(),
                rank: owner.player.book.effective_ranks[22].max(1),
                skill_id: 22,
                target,
                world_key: world_key.clone(),
            });
        }
    }
    for burn in secondary_deaths
        .iter()
        .flat_map(|death| &death.combat.ether_burns)
        .chain(&result.ether_burns)
    {
        if let Some(owner) = players.iter().find(|row| row.owner_id == burn.owner_id)
            && let Some(target) = boneyard_native_secondary_target(&world.enemies, burn.target_id)
        {
            secondary.apply_ether_burn(NativeSecondaryEtherBurnInput {
                owner_id: burn.owner_id.clone(),
                rank: owner.player.book.effective_ranks[14].max(1),
                target,
                world_key: world_key.clone(),
            });
        }
    }
    if secondary.next_actor_id != previous_actor_id {
        enroll_native_secondary_painter_owners(&mut secondary.actors, painters);
    }
    for effect in secondary_deaths
        .iter()
        .flat_map(|death| &death.combat.target_effects)
        .chain(&result.target_effects)
    {
        apply_native_secondary_target_effect(
            &mut secondary.target_effects,
            &effect.world_key,
            effect.target_id,
            effect.patch.clone(),
        );
    }
    result
}

/// `boneyardNativeSecondaryTarget`: only living, currently hostile bodies can
/// receive burn ownership; the inherited actor light registration is retained.
#[must_use]
pub fn boneyard_native_secondary_target(
    enemies: &EnemyWorld,
    target_id: usize,
) -> Option<NativeSecondaryTarget> {
    if let Some(actor) = enemies.actors.iter().find(|actor| actor.id == target_id) {
        return secondary_actor_target(actor);
    }
    let maggot = enemies.maggots.iter().find(|actor| actor.id == target_id)?;
    secondary_maggot_target(maggot)
}

/// `boneyardNativeSecondaryTargets`: the radial body gather is inclusive and
/// returns numeric actor order, independently of native primary cell ordering.
#[must_use]
pub fn boneyard_native_secondary_targets(
    enemies: &EnemyWorld,
    center: Vec2,
    radius: f64,
) -> Vec<NativeSecondaryTarget> {
    assert!(
        radius.is_finite() && radius >= 0.0,
        "secondary target radius must be finite and nonnegative"
    );
    let actor_targets = enemies.actors.iter().filter_map(secondary_actor_target);
    let maggot_targets = enemies.maggots.iter().filter_map(secondary_maggot_target);
    let mut targets: Vec<_> = actor_targets
        .chain(maggot_targets)
        .filter(|target| {
            hypot(target.position.x - center.x, target.position.y - center.y)
                <= radius + target.radius
        })
        .collect();
    targets.sort_by_key(|target| target.id);
    targets
}

fn secondary_actor_target(actor: &EnemyActor) -> Option<NativeSecondaryTarget> {
    if actor.life_state != EnemyLifeState::Alive || boneyard_enemy_actor_flags(actor) & 0x2 == 0 {
        return None;
    }
    Some(NativeSecondaryTarget {
        family: actor.config.enemy_token.id().into(),
        id: actor.id,
        light_registration: PainterRegistration {
            manager_lane: WorldManagerLane::Actor,
            registration_ordinal: actor.registration_ordinal,
        },
        native_flags: Some(0x2),
        registration_order: Some(actor.native_registration_order),
        position: actor.position,
        radius: boneyard_enemy_collision_radius(actor),
        scale: actor.config.scale,
        shield_health: actor.shield_health,
    })
}

fn secondary_maggot_target(maggot: &EnemyMaggotActor) -> Option<NativeSecondaryTarget> {
    if maggot.life_state != EnemyLifeState::Alive || !maggot.combat_active {
        return None;
    }
    Some(NativeSecondaryTarget {
        family: "MAGGOT".into(),
        id: maggot.id,
        light_registration: PainterRegistration {
            manager_lane: WorldManagerLane::Actor,
            registration_ordinal: maggot.registration_ordinal,
        },
        native_flags: Some(0x2),
        registration_order: Some(maggot.native_registration_order),
        position: maggot.position,
        radius: maggot.collision_radius,
        scale: 1.0,
        shield_health: 0.0,
    })
}

/// `createBoneyardSceneryTargets` and the Arena's source cleanup. Fencepost
/// registration counts shared endpoints before any fence bodies.
#[must_use]
pub fn boneyard_primary_scenery_targets(world: &BoneyardWorld) -> Vec<PrimarySpellTarget> {
    let scene = &world.loaded.scene;
    let cleanup = world.cleanup_bounds();
    let mut targets = Vec::new();
    for (registration_order, object) in scene.objects.iter().enumerate() {
        let radius = match object.type_id {
            2001 => 8.0,
            2009 | 2040 => 1.0,
            2029 => 0.01,
            _ => continue,
        };
        if cleanup.is_some_and(|bounds| !inside_bounds(object.position, bounds)) {
            continue;
        }
        let id = format!("scenery:{}", object.eid);
        if !world.retains_primary_scenery_target(&id) {
            continue;
        }
        targets.push(scenery_target(
            id,
            object.position,
            radius,
            registration_order,
            if object.type_id == 2029 {
                PrimarySpellTargetKind::Gravestone
            } else {
                PrimarySpellTargetKind::Scenery
            },
        ));
    }
    for (index, position) in
        native_boneyard_fence_post_positions(&scene.fences).into_iter().enumerate()
    {
        let id = format!("fencepost:{index}");
        if !world.retains_primary_scenery_target(&id) {
            continue;
        }
        targets.push(scenery_target(
            id,
            position,
            10.0,
            scene.objects.len() + index,
            PrimarySpellTargetKind::Scenery,
        ));
    }
    targets
}

fn scenery_target(
    id: String,
    position: Vec2,
    radius: f64,
    order: usize,
    kind: PrimarySpellTargetKind,
) -> PrimarySpellTarget {
    PrimarySpellTarget {
        active: true,
        actor_flags: 4,
        attachment: Vec2::ZERO,
        body_radius: radius,
        cell_binding_order: order,
        heading_degrees: None,
        grid_position: None,
        id,
        kind,
        native_priority: 1000,
        pending_remove: false,
        position,
        query_lane: PrimarySpellQueryLane::Grid,
        registration_order: order,
    }
}
fn inside_bounds(point: Vec2, bounds: BoneyardBounds) -> bool {
    point.x >= bounds.x
        && point.y >= bounds.y
        && point.x <= bounds.x + bounds.width
        && point.y <= bounds.y + bounds.height
}

pub(crate) struct BoneyardPrimaryGeometry<'a> {
    pub bounds: BoneyardBounds,
    pub collision: &'a BoneyardCollisionWorld,
    pub static_collision: &'a BoneyardCollisionWorld,
    pub prepared: &'a NavigationSet,
    pub gravestones: &'a [&'a str],
}

impl BoneyardPrimaryGeometry<'_> {
    pub fn can_place(&self, position: Vec2, radius: f64) -> bool {
        can_place_boneyard_body(position, self.bounds, self.collision, radius)
    }

    pub fn obstruction(
        &self,
        start: Vec2,
        end: Vec2,
        excluded: Option<&str>,
        mask: u32,
    ) -> Option<Vec2> {
        first_boneyard_line_obstruction(
            start,
            end,
            self.bounds,
            self.collision,
            excluded,
            mask,
            &[],
        )
    }

    pub fn can_traverse(
        &self,
        spell: PrimarySpellTraversalActor<'_>,
        from: Vec2,
        to: Vec2,
        radius: f64,
        mask: u32,
    ) -> bool {
        if radius == 0.0 {
            return self.obstruction(from, to, None, mask).is_none();
        }
        let ignored = if matches!(
            spell,
            PrimarySpellTraversalActor::Projectile(PrimarySpellProjectile::Earth(_))
                | PrimarySpellTraversalActor::EtherealBoulder(_)
        ) {
            self.gravestones
        } else {
            &[]
        };
        first_boneyard_path_block_progress(from, to, self.bounds, self.collision, radius, ignored)
            .is_none()
    }
}
impl EnemyNavigation for BoneyardPrimaryGeometry<'_> {
    fn direct_path_clear(&self, start: Vec2, end: Vec2, _: f64, _: f64) -> bool {
        first_boneyard_path_block_progress(start, end, self.bounds, self.collision, 0.0, &[])
            .is_none()
    }
    fn find_route(
        &self,
        start: Vec2,
        end: Vec2,
        clearance: f64,
        body_radius: f64,
    ) -> Option<Vec<Vec2>> {
        self.prepared.find_route(
            RouteRequest {
                body_radius,
                bounds: self.bounds,
                clearance,
                end,
                end_body_radius: None,
                ignored_source_ids: &[],
                start,
            },
            self.static_collision,
        )
    }
}
