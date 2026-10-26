//! Complete projectile trajectories share the constructor and event projections above.
use super::*;
use solomon_sim::enemies::model::*;
use solomon_sim::enemies::projectile_emission::{SpawnEnemyProjectileOptions, spawn_projectile};
use solomon_sim::enemies::projectiles::*;
use solomon_sim::enemies::world::*;
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::spells::model::{FirePatchType, PrimarySpellFirePatch};
use std::cell::{Cell, RefCell};

struct ProjectileFixtureGeometry<'a> {
    name: &'a str,
    tick: Cell<u64>,
    queries: &'a [Value],
    next_query: Cell<usize>,
    explosions: RefCell<Vec<Value>>,
}
impl solomon_sim::enemies::path::EnemyNavigation for ProjectileFixtureGeometry<'_> {
    fn direct_path_clear(&self, _start: Vec2, _end: Vec2, _clearance: f64, _radius: f64) -> bool {
        true
    }
    fn find_route(
        &self,
        _start: Vec2,
        _end: Vec2,
        _clearance: f64,
        _radius: f64,
    ) -> Option<Vec<Vec2>> {
        None
    }
}
impl EnemyGeometry for ProjectileFixtureGeometry<'_> {
    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2 {
        request.requested_position
    }
    fn line_obstruction(&self, _start: Vec2, _end: Vec2, _mask: u32) -> Option<Vec2> {
        None
    }
    fn projectile_world_blocked(&self, query: EnemyProjectileWorldQuery) -> bool {
        let request = match query {
            EnemyProjectileWorldQuery::Line {
                projectile_id,
                start,
                end,
                native_exclusion_mask,
            } => json!({"kind":"line",
                "projectileId":projectile_id,"start":point_json(start),"end":point_json(end),"nativeExclusionMask":native_exclusion_mask,"radius":0}),
            EnemyProjectileWorldQuery::Point { projectile_id, position, radius } => {
                json!({"kind":"point","projectileId":projectile_id,"position":point_json(position),"radius":radius})
            }
            EnemyProjectileWorldQuery::Bounds { projectile_id, position, margin } => {
                json!({"kind":"bounds","projectileId":projectile_id,"position":point_json(position),"margin":margin})
            }
            EnemyProjectileWorldQuery::View { projectile_id, position, margin } => {
                json!({"kind":"view","projectileId":projectile_id,"position":point_json(position),"margin":margin})
            }
        };
        let index = self.next_query.get();
        let expected = self
            .queries
            .get(index)
            .unwrap_or_else(|| panic!("{} unexpected query {index}", self.name));
        assert_json_equal(&request, &expected["request"], &format!("{} query {index}", self.name));
        assert_eq!(self.tick.get(), expected["tick"].as_u64().unwrap(), "{} query tick", self.name);
        self.next_query.set(index + 1);
        expected["blocked"].as_bool().unwrap()
    }
    fn on_projectile_explosion(&self, position: Vec2) {
        self.explosions
            .borrow_mut()
            .push(json!({"tick":self.tick.get(),"position":point_json(position)}));
    }
}

pub(super) fn fire_patch_json(patch: &PrimarySpellFirePatch) -> Value {
    let native_type = match patch.native_type {
        FirePatchType::Fire => "fire",
        FirePatchType::GoodGuy => "goodguy",
        FirePatchType::Moving => "moving",
    };
    json!({"ageTicks":patch.common.age_ticks,"atlasPhase":patch.atlas_phase,"atlasPhaseStep":patch.atlas_phase_step,
        "burnDamage":patch.burn_damage,"damage":patch.damage,"drawAlpha":patch.draw_alpha,"fadeAlpha":patch.fade_alpha,
        "id":patch.common.id,"kind":"fire-patch","life":patch.life,"nativeType":native_type,"ownerId":patch.common.owner_id,
        "painterRegistrations":patch.common.painter_registrations.iter().copied().map(painter_registration_json).collect::<Vec<_>>(),
        "position":point_json(patch.position),"scale":patch.scale,"horizontalSign":patch.horizontal_sign,
        "supplementalContact":patch.supplemental_contact,"velocity":point_json(patch.velocity),"velocityMultiplier":point_json(patch.velocity_multiplier),
        "worldKey":patch.common.world_key})
}

pub(super) fn projectile_effect_json(effect: &EnemyProjectileEffect) -> Value {
    let state = &effect.state;
    let mut record = json!({"ageTicks":state.age_ticks,"alpha":state.alpha,"alphaLossPerTick":state.alpha_loss_per_tick,
        "angularVelocityDeg":state.angular_velocity_deg,"atlas":state.atlas.id(),"blendMode":state.blend_mode.id(),"entry":state.entry,"id":state.id,
        "kind":effect.kind.id(),"lastStepTick":state.last_step_tick,"lightRegistration":state.light_registration.map(painter_registration_json),
        "lifetimeTicks":state.lifetime_ticks,"ownerActorId":state.owner_actor_id,"ownerProjectileId":state.owner_projectile_id,
        "painterRegistration":painter_registration_json(state.painter_registration),"phaseOriginTicks":state.phase_origin_ticks,"position":point_json(state.position),
        "rotationDeg":state.rotation_deg,"scale":state.scale,"spawnTick":state.spawn_tick,"tint":state.tint,"velocity":point_json(state.velocity)});
    match &effect.kind {
        EnemyProjectileEffectKind::DemonFire { fire } => record["fire"] = fire_patch_json(fire),
        EnemyProjectileEffectKind::PoisonBubble { growth_per_tick, maximum_scale } => {
            record["growthPerTick"] = json!(growth_per_tick);
            record["maximumScale"] = json!(maximum_scale);
        }
        _ => {}
    }
    record
}

fn payload(value: &str) -> EnemyProjectilePayload {
    match value {
        "cold" => EnemyProjectilePayload::Cold,
        "fire" => EnemyProjectilePayload::Fire,
        "none" => EnemyProjectilePayload::None,
        "normal" => EnemyProjectilePayload::Normal,
        "poison" => EnemyProjectilePayload::Poison,
        other => panic!("unknown projectile payload {other}"),
    }
}
fn projectile_kind(value: &str) -> EnemyProjectileKind {
    match value {
        "arrow" => EnemyProjectileKind::Arrow,
        "firebolt" => EnemyProjectileKind::Firebolt,
        "guided-missile" => EnemyProjectileKind::GuidedMissile,
        "demon-bomb" => EnemyProjectileKind::DemonBomb,
        "poison-pool" => EnemyProjectileKind::PoisonPool,
        other => panic!("unknown projectile kind {other}"),
    }
}
fn player_knockback_json(knockback: &EnemyPlayerKnockback) -> Value {
    json!({"actorId":knockback.actor_id,"delta":point_json(knockback.delta),"eventId":knockback.event_id,"playerId":knockback.player_id})
}

fn assert_projectile_sample(
    world: &EnemyWorld,
    combat: &solomon_sim::NativeRng,
    order: &WorldPainterOrder,
    sample: &Value,
    label: &str,
) {
    let projectiles: Vec<_> = world.projectiles.iter().map(enemy_projectile_json).collect();
    assert_json_equal(
        &json!(projectiles),
        &sample["projectiles"],
        &format!("{label} projectile rows"),
    );
    assert_eq!(
        world.projectile_effects.len(),
        sample["effects"].as_array().unwrap().len(),
        "{label} effect count"
    );
    for (effect, expected) in
        world.projectile_effects.iter().zip(sample["effects"].as_array().unwrap())
    {
        let mut actual = projectile_effect_json(effect);
        if matches!(effect.kind, EnemyProjectileEffectKind::FireboltTrail) {
            for axis in ["x", "y"] {
                assert_trig(
                    number(&actual["position"], axis),
                    number(&expected["position"], axis),
                    label,
                );
            }
            actual["position"] = expected["position"].clone();
        }
        assert_json_equal(&actual, expected, &format!("{label} effect {}", effect.state.id));
    }
    assert_json_equal(
        &json!(world.events.iter().map(enemy_event_json).collect::<Vec<_>>()),
        &sample["events"],
        &format!("{label} events"),
    );
    assert_json_equal(
        &json!(world.player_damage.iter().map(player_damage_json).collect::<Vec<_>>()),
        &sample["damage"],
        &format!("{label} damage"),
    );
    assert_json_equal(
        &json!(world.player_knockbacks.iter().map(player_knockback_json).collect::<Vec<_>>()),
        &sample["knockbacks"],
        &format!("{label} knockbacks"),
    );
    let retained:Vec<_>=world.projectile_knockbacks.iter().map(|knockback| {
        json!({"actorId":knockback.actor_id,"delta":point_json(knockback.delta),"eventId":knockback.event_id,"playerId":knockback.player_id,
            "lastStepTick":knockback.last_step_tick,"remainingTicks":knockback.remaining_ticks})
    }).collect();
    assert_json_equal(&json!(retained), &sample["retainedKnockbacks"], label);
    let bindings: serde_json::Map<String, Value> = world
        .target_cell_bindings
        .iter()
        .map(|(id, binding)| {
            (
                id.clone(),
                json!({"cellX":binding.cell_x,"cellY":binding.cell_y,"order":binding.order}),
            )
        })
        .collect();
    assert_json_equal(&json!(bindings), &sample["bindings"], label);
    assert_json_equal(&json!(world.rng_state), &sample["rng"], &format!("{label} actor RNG"));
    assert_json_equal(
        &json!(rng_record(&world.steering_rng)),
        &sample["steering"],
        &format!("{label} steering RNG"),
    );
    assert_json_equal(
        &json!(rng_record(combat)),
        &sample["combat"],
        &format!("{label} combat RNG"),
    );
    for (actual, key) in [
        (world.next_projectile_id, "nextProjectileId"),
        (world.next_projectile_effect_id, "nextProjectileEffectId"),
        (world.next_event_id, "nextEventId"),
        (world.next_native_cell_binding_order, "nextNativeCellBindingOrder"),
        (world.next_native_registration_order, "nextNativeRegistrationOrder"),
    ] {
        assert_json_equal(&json!(actual), &sample[key], label);
    }
    assert_json_equal(
        &json!({"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}),
        &sample["painterOrder"],
        label,
    );
}

#[test]
fn projectile_world_queries_contacts_hazards_and_retirement_match_the_website() {
    let goldens: Value = serde_json::from_str(&fixture("enemy-projectiles-goldens.json")).unwrap();
    for case in goldens["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let geometry = ProjectileFixtureGeometry {
            name,
            tick: Cell::new(10),
            queries: case["queries"].as_array().unwrap(),
            next_query: Cell::new(0),
            explosions: RefCell::new(Vec::new()),
        };
        let players: Vec<_> = case["playerOrder"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| {
                let id = id.as_str().unwrap();
                let player = &case["players"][id];
                EnemyTarget {
                    id: id.to_owned(),
                    summoned: player["summoned"].as_bool().unwrap_or(false),
                    alive: boolean(player, "alive"),
                    collision_radius: number(player, "collisionRadius"),
                    connected: boolean(player, "connected"),
                    eligible: boolean(player, "eligible"),
                    heading_deg: None,
                    position: point_from_json(&player["position"]),
                    velocity_per_tick: point_from_json(&player["velocityPerTick"]),
                }
            })
            .collect();
        let mut world = EnemyWorld::new(case["seed"].as_str().unwrap(), 50);
        let mut order = WorldPainterOrder { next_actor_ordinal: 100, next_transient_ordinal: 70 };
        world.spawn(
            &[spawn_intent(&case["intent"])],
            EnemySpawnContext {
                players: &players,
                tick: 10,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &geometry,
            Some(&mut order),
        );
        let owner = world.actors.get(0).unwrap().snapshot();
        let input = &case["options"];
        spawn_projectile(
            &mut world,
            &owner,
            10,
            projectile_kind(case["kind"].as_str().unwrap()),
            8.0,
            SpawnEnemyProjectileOptions {
                cold_slow_ticks: input["coldSlowTicks"].as_u64(),
                heading_deg: input["headingDeg"].as_f64(),
                lifetime_ticks: input["lifetimeTicks"].as_u64(),
                minimum_speed: input["minimumSpeed"].as_f64(),
                payload: input["payload"].as_str().map(payload),
                poison_damage: input["poisonDamage"].as_f64(),
                poison_duration: input["poisonDuration"].as_f64(),
                position: None,
                secondary_damage: input["secondaryDamage"].as_f64(),
                speed: input["speed"].as_f64(),
                visual_phase_deg: None,
            },
            true,
            &mut order,
        );
        if case["alternateTargets"].as_bool() == Some(true) {
            world.projectiles.get_mut(0).unwrap().state_mut().target_player_id = None;
        }
        world.last_step_tick = Some(10);
        let mut combat = solomon_sim::NativeRng::new(801);
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        assert_projectile_sample(
            &world,
            &combat,
            &order,
            samples.next().unwrap(),
            &format!("{name} birth"),
        );
        for tick in 11..=10 + case["ticks"].as_u64().unwrap() {
            geometry.tick.set(tick);
            world.events.clear();
            world.player_damage.clear();
            world.player_knockbacks.clear();
            if case["tumble"].as_bool() == Some(true) {
                if tick == 18 {
                    assert!(world.set_arrow_chill_tumble_accumulator(1, 0.75));
                }
                if tick == 20 || tick == 21 {
                    assert_eq!(
                        world.tumble_arrow(
                            1,
                            Vec2::new(2.0, -1.0),
                            tick,
                            &mut combat,
                            Some(&mut order)
                        ),
                        tick == 20
                    );
                }
            }
            world.bind_enemy_targets(&players);
            world.step_projectiles(
                &EnemyStepContext {
                    enhanced_effects: true,
                    paused: false,
                    tick,
                    players: &players,
                    geometry: &geometry,
                    ability_effects: &[],
                    arena_scalars: EnemyArenaScalars::default(),
                    dialogue_busy: false,
                    roll_loot_seed: None,
                    native_visibility: None,
                    projected_point_visible: None,
                    native_view_bounds: None,
                    puppet_targets: &[],
                    light_at: None,
                    native_movement_view: None,
                },
                &mut order,
            );
            world.last_step_tick = Some(tick);
            if let Some(sample) =
                samples.peek().filter(|sample| sample["tick"].as_u64() == Some(tick))
            {
                assert_projectile_sample(
                    &world,
                    &combat,
                    &order,
                    sample,
                    &format!("{name} tick {tick}"),
                );
                samples.next();
            }
        }
        assert!(samples.next().is_none(), "all {name} samples consumed");
        assert_eq!(
            geometry.next_query.get(),
            geometry.queries.len(),
            "all {name} world queries consumed"
        );
        assert_json_equal(
            &json!(*geometry.explosions.borrow()),
            &case["explosions"],
            &format!("{name} explosion callbacks"),
        );
    }
}
