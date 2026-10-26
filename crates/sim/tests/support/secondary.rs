//! Shared source-fixture readers and snapshots for the secondary replay tests.

use serde_json::{Value, json};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::secondary::golem::*;
use solomon_sim::secondary::*;
use solomon_sim::spells::casting::NativePlayerWeaponKind;
use solomon_sim::{HubWizard, NativeRng, Vec2};

pub fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
pub fn point_snapshot(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}
pub fn rng_snapshot(value: &NativeRng) -> Value {
    json!({"indexA":value.index_a,"indexB":value.index_b,"words":&value.words[..]})
}
pub fn registration_snapshot(value: PainterRegistration) -> Value {
    json!({"managerLane":match value.manager_lane {WorldManagerLane::Actor=>"actor",WorldManagerLane::Transient=>"transient"},"registrationOrdinal":value.registration_ordinal})
}
pub fn registration(value: &Value) -> PainterRegistration {
    PainterRegistration {
        manager_lane: match value["managerLane"].as_str().unwrap() {
            "actor" => WorldManagerLane::Actor,
            "transient" => WorldManagerLane::Transient,
            other => panic!("unknown lane {other}"),
        },
        registration_ordinal: value["registrationOrdinal"].as_u64().unwrap() as usize,
    }
}

pub fn weapon(value: &Value) -> Option<NativePlayerWeaponKind> {
    match value.as_str() {
        Some("staff") => Some(NativePlayerWeaponKind::Staff),
        Some("wand") => Some(NativePlayerWeaponKind::Wand),
        None => None,
        other => panic!("unknown weapon {other:?}"),
    }
}
pub fn read_player(value: &Value) -> NativeSecondaryPlayerState {
    let mut player = NativeSecondaryPlayerState {
        cast_action: value["castAction"].as_object().map(|_| NativeSecondaryCastAction {
            weapon_kind: weapon(&value["castAction"]["weaponKind"]),
            progress: value["castAction"]["progress"].as_f64().unwrap(),
        }),
        cast_sequence: value["castSequence"].as_u64().unwrap(),
        cast_spin_ticks_remaining: value["castSpinTicksRemaining"].as_u64().unwrap(),
        firewalker: value["firewalker"].as_bool().unwrap(),
        fizzle_sequence: value["fizzleSequence"].as_u64().unwrap(),
        global_cooldown_ticks: value["globalCooldownTicks"].as_f64().unwrap(),
        held_slot: value["heldSlot"].as_u64().map(|id| id as usize),
        last_skill_id: value["lastSkillId"]
            .as_u64()
            .map(|id| NativeSecondaryAbilityId::from_skill_id(id as usize).unwrap()),
        magic_shield_absorb: value["magicShieldAbsorb"].as_f64().unwrap(),
        magic_shield_explosion_damage: value["magicShieldExplosionDamage"].as_f64().unwrap(),
        magic_shield_maximum: value["magicShieldMaximum"].as_f64().unwrap(),
        magic_shield_pulse_ticks: value["magicShieldPulseTicks"].as_u64().unwrap(),
        mindstar: value["mindstar"].as_bool().unwrap(),
        plane_orb_held: value["planeOrbHeld"].as_bool().unwrap(),
        planewalker_ticks_remaining: value["planewalkerTicksRemaining"].as_u64().unwrap(),
        regenerate: value["regenerate"].as_bool().unwrap(),
        reserved_mana: value["reservedMana"].as_f64().unwrap(),
        stoneskin_ticks_remaining: value["stoneskinTicksRemaining"].as_u64().unwrap(),
        ..NativeSecondaryPlayerState::default()
    };
    for id in 0..83 {
        player.cooldown_maximum_ticks_by_skill[id] =
            value["cooldownMaximumTicksBySkill"][id].as_f64().unwrap();
        player.cooldown_ticks_by_skill[id] = value["cooldownTicksBySkill"][id].as_f64().unwrap();
    }
    player
}
pub fn action_snapshot(action: Option<NativeSecondaryCastAction>) -> Value {
    match action {
        Some(action) => json!({"progress":action.progress,"weaponKind":match action.weapon_kind {
            Some(NativePlayerWeaponKind::Staff)=>Some("staff"),
            Some(NativePlayerWeaponKind::Wand)=>Some("wand"), None=>None,
        }}),
        None => Value::Null,
    }
}
pub fn player_snapshot(player: &NativeSecondaryPlayerState) -> Value {
    json!({
        "castAction":action_snapshot(player.cast_action),
        "castSequence":player.cast_sequence,"castSpinTicksRemaining":player.cast_spin_ticks_remaining,
        "cooldownMaximumTicksBySkill":&player.cooldown_maximum_ticks_by_skill[..],
        "cooldownTicksBySkill":&player.cooldown_ticks_by_skill[..],
        "firewalker":player.firewalker,"fizzleSequence":player.fizzle_sequence,
        "globalCooldownTicks":player.global_cooldown_ticks,"heldSlot":player.held_slot,
        "lastSkillId":player.last_skill_id.map(NativeSecondaryAbilityId::skill_id),
        "magicShieldAbsorb":player.magic_shield_absorb,"magicShieldExplosionDamage":player.magic_shield_explosion_damage,
        "magicShieldMaximum":player.magic_shield_maximum,"magicShieldPulseTicks":player.magic_shield_pulse_ticks,
        "mindstar":player.mindstar,"planeOrbHeld":player.plane_orb_held,
        "planewalkerTicksRemaining":player.planewalker_ticks_remaining,"regenerate":player.regenerate,
        "reservedMana":player.reserved_mana,"stoneskinTicksRemaining":player.stoneskin_ticks_remaining,
    })
}

pub fn effect_snapshot(effect: &NativeSecondaryTargetEffectState) -> Value {
    let electric=effect.electric_burn.as_ref().map(|burn|json!({"arcCount":burn.arc_count,"damagePerTick":burn.damage_per_tick,
        "ownerId":burn.owner_id,"sourceActorId":burn.source_actor_id,"stunFactor":burn.stun_factor,"ticks":burn.ticks}));
    let steamed=effect.steamed.as_ref().map(|burn|json!({"damagePerTick":burn.damage_per_tick,"emberDamage":burn.ember_damage,
        "emberFragments":burn.ember_fragments,"explodeDamage":burn.explode_damage,"explodeRadius":burn.explode_radius,
        "ownerId":burn.owner_id,"sourceActorId":burn.source_actor_id,"ticks":burn.ticks}));
    let mut order = Vec::new();
    for kind in &effect.movement_modifier_order {
        order.push(match kind {
            NativeSecondaryMovementModifierKind::ColdSlow => "cold-slow",
            NativeSecondaryMovementModifierKind::CircleSlow => "circle-slow",
            NativeSecondaryMovementModifierKind::Frozen => "frozen",
            NativeSecondaryMovementModifierKind::Stun => "stun",
            NativeSecondaryMovementModifierKind::Dazzle => "dazzle",
        });
    }
    json!({"circleSlowFactor":effect.circle_slow_factor,"circleSlowTicks":effect.circle_slow_ticks,
        "coldSlowFactor":effect.cold_slow_factor,"coldSlowMaterial":effect.cold_slow_material,"coldSlowTicks":effect.cold_slow_ticks,
        "dazzleMaximumTicks":effect.dazzle_maximum_ticks,"dazzleTicks":effect.dazzle_ticks,"disruptedTicks":effect.disrupted_ticks,
        "electricBurn":electric,"fleeTicks":effect.flee_ticks,"frostBurnDamagePerTick":effect.frost_burn_damage_per_tick,
        "frostBurnOwnerId":effect.frost_burn_owner_id,"frostBurnSkillId":effect.frost_burn_skill_id,
        "frostBurnSourceActorId":effect.frost_burn_source_actor_id,"frostBurnTicks":effect.frost_burn_ticks,
        "frozenTicks":effect.frozen_ticks,"frozenTimeScale":effect.frozen_time_scale,"movementModifierOrder":order,
        "prismaticTicks":effect.prismatic_ticks,"stunFactor":effect.stun_factor,"stunTicks":effect.stun_ticks,
        "steamed":steamed,"targetId":effect.target_id,"timeScale":effect.time_scale,"weakenFactor":effect.weaken_factor,
        "worldKey":effect.world_key})
}

pub fn assert_snapshot(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}")
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_snapshot(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} fields");
            for (key, expected) in expected {
                assert_snapshot(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

pub fn golem_snapshot(value: NativeSecondaryGolemState) -> Value {
    json!({
    "actionHeadingOffsetDegrees":value.action_heading_offset_degrees,
    "actionDurationTicks":value.action_duration_ticks,
    "actionTick":value.action_tick,
    "circleSlowTicks":value.circle_slow_ticks,
    "currentHealth":value.current_health,
    "damageMaximum":value.damage_maximum,
    "iron":value.iron,
    "gaitTick":value.gait_tick,
    "leftConnectorOffset":point_snapshot(value.left_connector_offset),
    "leftFoot":point_snapshot(value.left_foot),
    "leftFootBob":point_snapshot(value.left_foot_bob),
    "leftFootNext":point_snapshot(value.left_foot_next),
    "leftFootPrevious":point_snapshot(value.left_foot_previous),
    "leftFootProgress":value.left_foot_progress,
    "leftFootRotationDegrees":value.left_foot_rotation_degrees,
    "leftLimbMode":value.left_limb_mode,
    "maximumHealth":value.maximum_health,
    "orbitDirection":value.orbit_direction,
    "orbitHeadingRadians":value.orbit_heading_radians,
    "phase":match value.phase {NativeGolemPhase::Active=>"active",NativeGolemPhase::Assembly=>"assembly",NativeGolemPhase::Attack=>"attack",NativeGolemPhase::Provoke=>"provoke"},
    "poseVariant":value.pose_variant,
    "provokeRollBound":value.provoke_roll_bound,
    "reflectFactor":value.reflect_factor,
    "rightConnectorOffset":point_snapshot(value.right_connector_offset),
    "rightFoot":point_snapshot(value.right_foot),
    "rightFootBob":point_snapshot(value.right_foot_bob),
    "rightFootNext":point_snapshot(value.right_foot_next),
    "rightFootPrevious":point_snapshot(value.right_foot_previous),
    "rightFootProgress":value.right_foot_progress,
    "rightFootRotationDegrees":value.right_foot_rotation_degrees,
    "rightLimbMode":value.right_limb_mode,
    "targetPollTicksRemaining":value.target_poll_ticks_remaining,
    })
}

pub fn actor_snapshot(value: &impl SecondaryActorRead) -> Value {
    json!({
    "ageTicks":value.age_ticks,
    "alpha":value.alpha,
    "damage":value.damage,
    "enhanced":value.enhanced,
    "endpoint":point_snapshot(value.endpoint),
    "frame":value.frame,
    "freezeTicks":value.freeze_ticks,
    "golem":value.golem().copied().map(golem_snapshot),
    "hitTargetIds":value.hit_target_ids,
    "id":value.id,
    "kind":value.kind.as_str(),
    "lifetimeTicks":value.lifetime_ticks,
    "lightRegistration":value.light_registration.map(registration_snapshot),
    "midpoint":point_snapshot(value.midpoint),
    "miscLightAppendOrdinal":value.misc_light_append_ordinal,
    "ownerId":value.owner_id,
    "painterRegistrations":value.painter_registrations.iter().copied().map(registration_snapshot).collect::<Vec<_>>(),
    "phase":value.phase,
    "position":point_snapshot(value.position),
    "presentationRng":value.presentation_rng.as_ref().map(rng_snapshot),
    "quantity":value.quantity,
    "radius":value.radius,
    "rank":value.rank,
    "rotationRadians":value.rotation_radians,
    "scale":value.scale,
    "skillId":value.skill_id,
    "slowFactor":value.slow_factor,
    "targetId":value.target_id,
    "variant":value.variant,
    "velocity":point_snapshot(value.velocity),
    "worldKey":value.world_key,
    })
}

pub fn flash_snapshot(value: NativeSecondaryScreenFlashState) -> Value {
    json!({
    "alpha":value.alpha,
    "blue":value.blue,
    "decayPerTick":value.decay_per_tick,
    "green":value.green,
    "pointAttenuated":value.point_attenuated,
    "red":value.red,
    })
}

pub fn event_snapshot(value: &NativeSecondaryEventState) -> Value {
    json!({
    "actorId":value.actor_id,
    "cameraDisplacement":value.camera_displacement.map(point_snapshot),
    "cameraMagnitude":value.camera_magnitude,
    "cue":value.cue.map(NativeSecondaryAudioCue::as_str),
    "eventId":value.event_id,
    "gain":value.gain,
    "kind":value.kind.as_str(),
    "ownerId":value.owner_id,
    "pitch":value.pitch,
    "position":point_snapshot(value.position),
    "screenFlash":value.screen_flash.map(flash_snapshot),
    "skillId":value.skill_id,
    "tick":value.tick,
    "worldKey":value.world_key,
    })
}

pub fn state_snapshot(state: &NativeSecondarySimulationState) -> Value {
    let actors: Vec<_> = state.actors.iter().map(|actor| actor_snapshot(&actor)).collect();
    let events: Vec<_> = state.events.iter().map(event_snapshot).collect();
    let mut players = json!({});
    for (id, player) in &state.players {
        players[id] = player_snapshot(player);
    }
    let effects: Vec<_> = state.target_effects.iter().map(effect_snapshot).collect();
    json!({"actors":actors,"events":events,"firewalkerGeometrySequence":state.firewalker_geometry_sequence,
 "nextActorId":state.next_actor_id,"nextEventId":state.next_event_id,"players":players,"rng":rng_snapshot(&state.rng),"targetEffects":effects})
}
struct OpenWorld;
impl NativeGolemWorld for OpenWorld {
    fn resolve_movement(&mut self, requested: Vec2) -> Vec2 {
        requested
    }
}
pub fn read_state(value: &Value) -> NativeSecondarySimulationState {
    let mut state = NativeSecondarySimulationState::new(0);
    state.next_actor_id = value["nextActorId"].as_u64().unwrap();
    state.next_event_id = value["nextEventId"].as_u64().unwrap();
    state.firewalker_geometry_sequence =
        value["firewalkerGeometrySequence"].as_u64().unwrap() as u32;
    state.rng.index_a = value["rng"]["indexA"].as_u64().unwrap() as usize;
    state.rng.index_b = value["rng"]["indexB"].as_u64().unwrap() as usize;
    for (word, expected) in
        state.rng.words.iter_mut().zip(value["rng"]["words"].as_array().unwrap())
    {
        *word = expected.as_u64().unwrap() as u32;
    }
    for (id, player) in value["players"].as_object().unwrap() {
        state.players.insert(id.clone(), read_player(player));
    }
    for value in value["actors"].as_array().unwrap() {
        assert_eq!(value["kind"].as_str(), Some("golem"));
        let mut actor = NativeSecondaryActorState::new(
            value["id"].as_u64().unwrap(),
            NativeSecondaryActorKind::Golem,
            value["ownerId"].as_str().unwrap().to_owned(),
            value["worldKey"].as_str().unwrap().to_owned(),
            Some(45),
        );
        actor.age_ticks = value["ageTicks"].as_u64().unwrap();
        actor.damage = value["damage"].as_f64().unwrap();
        actor.lifetime_ticks = value["lifetimeTicks"].as_u64().unwrap();
        actor.position = point(&value["position"]);
        actor.quantity = value["quantity"].as_f64().unwrap();
        actor.radius = value["radius"].as_f64().unwrap();
        actor.rotation_radians = value["rotationRadians"].as_f64().unwrap();
        actor.variant = value["variant"].as_i64().unwrap() as i32;
        let source = &value["golem"];
        let mut golem = NativeSecondaryGolemState::new(
            actor.position,
            actor.rotation_radians,
            NativeGolemBirthStats {
                health: source["maximumHealth"].as_f64().unwrap(),
                damage_maximum: source["damageMaximum"].as_f64().unwrap(),
                iron: source["iron"].as_bool().unwrap(),
                reflect_factor: source["reflectFactor"].as_f64().unwrap(),
                pose_variant: source["poseVariant"].as_u64().unwrap() as u32,
            },
            &mut OpenWorld,
        );
        golem.phase = NativeGolemPhase::Active;
        actor.golem = Some(golem);
        state.actors.push(actor);
    }
    assert!(value["events"].as_array().unwrap().is_empty());
    assert!(value["targetEffects"].as_array().unwrap().is_empty());
    state
}
pub fn target(value: &Value) -> NativeSecondaryTarget {
    NativeSecondaryTarget {
        family: value["family"].as_str().unwrap().to_owned(),
        id: value["id"].as_u64().unwrap() as usize,
        light_registration: registration(&value["lightRegistration"]),
        native_flags: value["nativeFlags"].as_u64().map(|flags| flags as u32),
        registration_order: value["registrationOrder"].as_u64().map(|order| order as usize),
        position: point(&value["position"]),
        radius: value["radius"].as_f64().unwrap(),
        scale: value["scale"].as_f64().unwrap(),
        shield_health: value["shieldHealth"].as_f64().unwrap(),
    }
}

use solomon_sim::belt::PlayerBeltComponent;

use solomon_sim::equipment::{NativeEquipmentModifiers, NativeEquipmentScalarTransform};
use solomon_sim::profile::NativeSkillBookState;
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
pub fn factors(value: &Value) -> NativeOffensiveSpellFactors {
    let equipment = value.get("equipment").map(|value| {
        let mut modifiers = NativeEquipmentModifiers {
            global_damage_flat: value["globalDamageFlat"].as_f64().unwrap(),
            global_damage_multiplier: value["globalDamageMultiplier"].as_f64().unwrap(),
            global_mana_cost_flat: value["globalManaCostFlat"].as_f64().unwrap(),
            global_mana_cost_multiplier: value["globalManaCostMultiplier"].as_f64().unwrap(),
            ..NativeEquipmentModifiers::default()
        };
        for id in 0..8 {
            modifiers.class_damage_flat[id] = value["classDamageFlat"][id].as_f64().unwrap();
            modifiers.class_damage_multiplier[id] =
                value["classDamageMultiplier"][id].as_f64().unwrap();
            modifiers.class_mana_cost_flat[id] = value["classManaCostFlat"][id].as_f64().unwrap();
            modifiers.class_mana_cost_multiplier[id] =
                value["classManaCostMultiplier"][id].as_f64().unwrap();
            modifiers.class_recharge[id] = NativeEquipmentScalarTransform {
                offset: value["classRecharge"][id]["offset"].as_f64().unwrap(),
                scale: value["classRecharge"][id]["scale"].as_f64().unwrap(),
            };
        }
        for id in 0..83 {
            modifiers.skill_damage_flat[id] = value["skillDamageFlat"][id].as_f64().unwrap();
            modifiers.skill_damage_multiplier[id] =
                value["skillDamageMultiplier"][id].as_f64().unwrap();
        }
        modifiers
    });
    NativeOffensiveSpellFactors {
        damage: value["damage"].as_f64().unwrap(),
        mana_cost: value["manaCost"].as_f64().unwrap(),
        global_flat_damage: value["globalFlatDamage"].as_f64().unwrap_or(0.0),
        global_mana_reduction: value["globalManaReduction"].as_f64().unwrap_or(0.0),
        equipment,
    }
}
pub fn read_book(value: &Value) -> NativeSkillBookState {
    let ranks = |key: &str| {
        value[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    };
    NativeSkillBookState {
        advanced_unlocks: value["advancedUnlocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_bool().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        discipline_root: value["disciplineRoot"].as_u64().unwrap() as usize,
        effective_ranks: ranks("effectiveRanks"),
        element_root: value["elementRoot"].as_u64().unwrap() as usize,
        learned_skill_order: value["learnedSkillOrder"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect(),
        permanent_ranks: ranks("permanentRanks"),
        primary_skill_id: value["primarySkillId"].as_u64().unwrap() as usize,
        weld_build_id: value["weldBuildId"].as_u64().map(|value| value as u32),
        weld_component_ranks: value["weldComponentRanks"].as_array().map(|values| {
            values
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        }),
    }
}

pub struct World<'a> {
    pub case: &'a Value,
    pub targets: Vec<NativeSecondaryTarget>,
}
impl NativeSecondaryWorld for World<'_> {
    fn target_exists(&self, world: &str, id: usize) -> bool {
        world == "boneyard:oracle" && self.targets.iter().any(|target| target.id == id)
    }
    fn dampen_candidates(&self, _world: &str, _origin: Vec2) -> NativeSecondaryDampenCandidates {
        let value = &self.case["dampen"];
        let mut projectiles = Vec::new();
        for value in value["projectiles"].as_array().unwrap() {
            projectiles.push(NativeSecondaryDampenProjectileCandidate {
                id: value["id"].as_u64().unwrap() as usize,
                kind: match value["kind"].as_str().unwrap() {
                    "firebolt" => NativeSecondaryDampenProjectileKind::Firebolt,
                    "guided-missile" => NativeSecondaryDampenProjectileKind::GuidedMissile,
                    "skull-missile" => NativeSecondaryDampenProjectileKind::SkullMissile,
                    "dark-fireball" => NativeSecondaryDampenProjectileKind::DarkFireball,
                    other => panic!("unknown projectile {other}"),
                },
                payload: match value["payload"].as_str().unwrap() {
                    "fire" => NativeSecondaryDampenPayload::Fire,
                    "cold" => NativeSecondaryDampenPayload::Cold,
                    "poison" => NativeSecondaryDampenPayload::Poison,
                    "dark" => NativeSecondaryDampenPayload::Dark,
                    other => panic!("unknown payload {other}"),
                },
                position: point(&value["position"]),
            });
        }
        NativeSecondaryDampenCandidates {
            caster_target_ids: value["casterTargetIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_u64().unwrap() as usize)
                .collect(),
            shield_target_ids: value["shieldTargetIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_u64().unwrap() as usize)
                .collect(),
            projectiles,
        }
    }
    fn phasing_destination(&self, _player: &str, _origin: Vec2, _direction: Vec2) -> Option<Vec2> {
        if self.case["mode"].as_str() == Some("blocked-phase") {
            None
        } else {
            Some(Vec2::new(20.0, 0.0))
        }
    }
    fn golem_movement(
        &self,
        _player: &str,
        _world: &str,
        _origin: Vec2,
        requested: Vec2,
        _radius: f64,
    ) -> Vec2 {
        requested
    }
    fn golem_placement(
        &self,
        _player: &str,
        _world: &str,
        requested: Vec2,
        _rng: &mut NativeRng,
    ) -> Vec2 {
        requested
    }
    fn teleport_destination(&self, _player: &str, _rng: &mut NativeRng) -> Vec2 {
        Vec2::new(0.0, 0.0)
    }
    fn knockback_area_movement(
        &mut self,
        world: &str,
        target_id: usize,
        delta: Vec2,
        _source_actor_id: u64,
    ) -> Vec<usize> {
        assert_eq!(self.case["movement"], "unobstructed-target-roots");
        if world == "boneyard:oracle"
            && let Some(target) = self.targets.iter_mut().find(|target| target.id == target_id)
        {
            // This declared kernel world has no walls or neighboring body response.
            // Actual body-pair propagation is replayed by the Boneyard-world fixture.
            target.position.x = solomon_sim::float::fround(target.position.x + delta.x);
            target.position.y = solomon_sim::float::fround(target.position.y + delta.y);
        }
        Vec::new()
    }
    fn target(&self, _world: &str, id: usize) -> Option<NativeSecondaryTarget> {
        self.targets.iter().find(|target| target.id == id).cloned()
    }
    fn targets(&self, _world: &str, center: Vec2, radius: f64) -> Vec<NativeSecondaryTarget> {
        self.targets
            .iter()
            .filter(|target| {
                solomon_sim::float::hypot(
                    target.position.x - center.x,
                    target.position.y - center.y,
                ) <= radius + target.radius
            })
            .cloned()
            .collect()
    }
    fn scenery_targets(
        &self,
        _world: &str,
        _center: Vec2,
        _radius: f64,
    ) -> Vec<NativeSecondarySceneryTarget> {
        self.case["scenery"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| NativeSecondarySceneryTarget {
                id: value["id"].as_u64().unwrap() as usize,
                position: point(&value["position"]),
                type_id: value["typeId"].as_u64().unwrap() as u32,
                secondary_variant: value["secondaryVariant"].as_u64().map(|value| value as u32),
            })
            .collect()
    }
}
pub fn cast_snapshot(result: &NativeSecondaryTickResult) -> Value {
    let mut relocated = json!({});
    for (id, position) in &result.relocated_players {
        relocated[id] = point_snapshot(*position);
    }
    // This retained fixture predates continuous headings and records only the body bank.
    let facing_indexes: std::collections::BTreeMap<_, _> = result
        .facing_heading_degrees_by_player
        .iter()
        .map(|(id, heading)| (id, solomon_sim::heading::heading_index(*heading)))
        .collect();
    json!({"dampenedCasterTargetIds":result.dampened_caster_target_ids,"dispelledShieldTargetIds":result.dispelled_shield_target_ids,
  "manaRecovered":result.mana_recovered,"manaUnderflowPlayerIds":result.mana_underflow_player_ids,"manaSpent":result.mana_spent,
  "facingHeadingIndexes":facing_indexes,"relocatedPlayers":relocated,"removedProjectileIds":result.removed_projectile_ids})
}

pub fn authority_from_fixture<'a>(
    declared: &'a Value,
    book: &'a NativeSkillBookState,
    belt: &'a PlayerBeltComponent,
    wizard: &'a HubWizard,
) -> NativeSecondaryPlayerAuthority<'a> {
    NativeSecondaryPlayerAuthority {
        // Historical authority fixtures predate the separate alive input; their
        // retained characters are alive unless the current fixture says otherwise.
        alive: declared["alive"].as_bool().unwrap_or(true),
        weapon_kind: weapon(&declared["weaponKind"]),
        belt,
        character: wizard,
        cold_slow_factor: declared["coldSlowFactor"].as_f64().unwrap(),
        current_mana: declared["currentMana"].as_f64().unwrap(),
        eligible: declared["eligible"].as_bool().unwrap(),
        enhanced_effects: declared["enhancedEffects"].as_bool().unwrap(),
        explosive_shield_damage: declared["explosiveShieldDamage"].as_f64().unwrap(),
        explosive_shield_raw_mana_cost: declared["explosiveShieldRawManaCost"].as_f64().unwrap(),
        fire_burn_damage: declared["fireBurnDamage"].as_f64().unwrap(),
        freeze_duration_multiplier: declared["freezeDurationMultiplier"].as_f64().unwrap(),
        focus_instant_recharge_chance_percent: declared["focusInstantRechargeChancePercent"]
            .as_f64()
            .unwrap(),
        golem_iron: declared["golemIron"].as_bool().unwrap(),
        golem_raw_mana_cost: declared["golemRawManaCost"].as_f64().unwrap(),
        golem_reflect_factor: declared["golemReflectFactor"].as_f64().unwrap(),
        input: NativeSecondaryInput {
            aim: declared["input"]["aim"].as_object().map(|_| point(&declared["input"]["aim"])),
            primary: false,
            quickbar: Some(0),
        },
        maximum_mana: declared["maximumMana"].as_f64().unwrap(),
        magic_storm_duration_bonus_ticks: declared["magicStormDurationBonusTicks"]
            .as_u64()
            .unwrap(),
        magic_storm_frequency_factor: declared["magicStormFrequencyFactor"].as_f64().unwrap(),
        magic_storm_raw_mana_cost: declared["magicStormRawManaCost"].as_f64().unwrap(),
        maximum_golem: declared["maximumGolem"].as_bool().unwrap(),
        maximum_leviathan: declared["maximumLeviathan"].as_bool().unwrap(),
        maximum_magic_storm: declared["maximumMagicStorm"].as_bool().unwrap(),
        maximum_ring_of_fire: declared["maximumRingOfFire"].as_bool().unwrap(),
        maximum_ring_of_ice: declared["maximumRingOfIce"].as_bool().unwrap(),
        mana_recovery_per_tick: declared["manaRecoveryPerTick"].as_f64().unwrap(),
        offensive_factors: factors(&declared["offensiveFactors"]),
        secondary_recharge_factor: declared["secondaryRechargeFactor"].as_f64().unwrap(),
        skill_book: book,
        world_key: declared["worldKey"].as_str().unwrap(),
    }
}
