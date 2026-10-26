//! Read-only accepted-state projection shared by single/multi-player Game replays.

use super::{player_combat, player_snapshot, secondary_state, session_snapshot, tutorial_snapshot};
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::loot::BoneyardLootEventType;
use solomon_sim::boneyard::loot::LootActorRead;
use solomon_sim::enemies::model::EnemyLifeState;
use solomon_sim::harden_effects::NativeHardenEffect;
use solomon_sim::loot::NativeLootDropKind;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::session::GameSession;
use solomon_sim::spells::model::{
    PrimarySpellProjectile, PrimarySpellProjectilePhase, PrimarySpellTransient,
};

fn point(position: Vec2) -> Value {
    json!([position.x, position.y])
}

fn registration(registration: PainterRegistration) -> Value {
    json!([
        match registration.manager_lane {
            WorldManagerLane::Actor => "actor",
            WorldManagerLane::Transient => "transient",
        },
        registration.registration_ordinal
    ])
}

fn transient_kind(effect: &PrimarySpellTransient) -> &'static str {
    match effect {
        PrimarySpellTransient::Harden(NativeHardenEffect::Shard { .. }) => "harden-shard",
        PrimarySpellTransient::Harden(NativeHardenEffect::Burst { .. }) => "harden-burst",
        PrimarySpellTransient::Staff(actor) => actor.kind(),
        PrimarySpellTransient::Weld(_) => unreachable!("these journeys cast the five elements"),
        PrimarySpellTransient::AirHurricane(_) => "air-hurricane",
        PrimarySpellTransient::WaterAura(_) => "water-aura",
        PrimarySpellTransient::WaterHail(_) => "water-hail",
        PrimarySpellTransient::Air(_) => "air",
        PrimarySpellTransient::Water(_) => "water",
        PrimarySpellTransient::EarthBoulderBit(_) => "earth-boulder-bit",
        PrimarySpellTransient::EarthImpact(_) => "earth-impact",
        PrimarySpellTransient::EarthCalledRock(_) => "earth-called-rock",
        PrimarySpellTransient::EtherImpact(_) => "ether-impact",
        PrimarySpellTransient::EtherPierceStreak(_) => "ether-pierce-streak",
        PrimarySpellTransient::EtherBlast(_) => "ether-blast",
        PrimarySpellTransient::FireParticle(_) => "fire",
        PrimarySpellTransient::FireImpact(_) => "fire-impact",
        PrimarySpellTransient::FireEmber(_) => "fire-ember",
        PrimarySpellTransient::FireExplosion(_) => "fire-explosion",
        PrimarySpellTransient::FireGoodImp(_) => "fire-good-imp",
        PrimarySpellTransient::FirePatch(_) => "fire-patch",
    }
}

fn primary_snapshot(session: &GameSession) -> Value {
    let projectiles: Vec<_> = session
        .spells
        .projectiles
        .iter()
        .map(|projectile| {
            let common = projectile.common();
            let kind = match projectile {
                PrimarySpellProjectile::Earth(_) => "earth",
                PrimarySpellProjectile::Ether(_) => "ether",
                PrimarySpellProjectile::Fire(_) => "fire",
                PrimarySpellProjectile::Weld(_) => "weld",
            };
            let phase = match common.phase {
                PrimarySpellProjectilePhase::Flight => "flight",
                PrimarySpellProjectilePhase::Held => "held",
            };
            json!([
                common.id,
                kind,
                common.age_ticks,
                point(common.position),
                point(common.velocity),
                common.damage,
                phase,
                common.world_key,
                registration(common.registration)
            ])
        })
        .collect();
    let transients: Vec<_> = session
        .spells
        .transients
        .iter()
        .map(|effect| {
            let common = effect.common();
            json!([
                common.id,
                transient_kind(effect),
                common.age_ticks,
                common.world_key,
                common.light_registration.map(registration),
                common.painter_registrations.iter().copied().map(registration).collect::<Vec<_>>()
            ])
        })
        .collect();
    json!({"nextId":session.spells.next_id,"projectiles":projectiles,"transients":transients})
}

fn world_snapshot(session: &GameSession, id: &str) -> Value {
    let Some(world) = &session.boneyard else {
        return json!({"kind":"hub","participant":session_snapshot::hub_participant(&session.participants[id]),
            "collisionRng":session.collision_rng,
            "students":session.npcs.students.students.iter().map(|student| json!([
                student.id,point(student.position),student.tick,student.rng_state,
                student.registration_ordinal])).collect::<Vec<_>>()});
    };
    let enemies = &world.enemies;
    let loot = &world.loot;
    let loot_events: Vec<_> = world
        .loot_events
        .iter()
        .map(|event| {
            let kind = match event.event_type {
                BoneyardLootEventType::GoodieKeyNeeded => "goodie-key-needed",
                BoneyardLootEventType::GoodiePhase => "goodie-phase",
                BoneyardLootEventType::LootDropSound => "loot-drop-sound",
                BoneyardLootEventType::LootPickup => "loot-pickup",
            };
            json!([event.event_id, kind, event.actor_id, event.player_id, event.text, event.tick])
        })
        .collect();
    let actors: Vec<_> = enemies
        .actors
        .iter()
        .map(|actor| {
            json!([
                actor.id,
                actor.config.enemy_token.id(),
                point(actor.position),
                actor.heading_deg,
                actor.current_health,
                match actor.life_state {
                    EnemyLifeState::Alive => "alive",
                    EnemyLifeState::Dying => "dying",
                },
                actor.registration_ordinal
            ])
        })
        .collect();
    let loot_actors: Vec<_> = loot
        .actors
        .iter()
        .map(|actor| {
            let kind = match actor.kind {
                NativeLootDropKind::Bonus => "bonus",
                NativeLootDropKind::Gold => "gold",
                NativeLootDropKind::Orb => "orb",
                NativeLootDropKind::Sack => "sack",
            };
            json!([
                actor.id,
                kind,
                point(actor.position),
                actor.age_ticks,
                actor.reward().amount,
                actor.reward().item.as_ref().map(|item| item.native_type_id)
            ])
        })
        .collect();
    json!({"kind":"boneyard","runId":world.loaded.run_id,
        "lantern":world.lantern_position.map(|position|json!({"x":position.x,"y":position.y})),
        "encounter":world.encounter.as_ref().map(|encounter|json!([encounter.phase.id(),
            encounter.run_event_id,encounter.voice_ticks_remaining,player_snapshot::rng_snapshot(&encounter.rng)])),
        "waves":world.waves.as_ref().map(|waves|json!([waves.state.phase.id(),waves.state.wave_ordinal,
            waves.state.low_population_ticks,waves.state.spawn_countdown,player_snapshot::rng_snapshot(&waves.state.rng)])),
        "tutorial":world.tutorial.as_ref().map(tutorial_snapshot::state_row),
        "feedback":{"accumulator":world.enemy_world_feedback.accumulator,"magnitude":world.enemy_world_feedback.magnitude},
        "score":session_snapshot::score(&session.participants[id].score),"ouchDeadline":world.player_ouch_deadline_tick,
        "actors":actors,
        "projectiles":enemies.projectiles.iter().map(|projectile| {
            let state=projectile.state();json!([state.id,projectile.kind().id(),point(state.position),
                state.age_ticks,registration(state.painter_registration)])
        }).collect::<Vec<_>>(),
        "events":world.enemy_events.iter().map(|event|json!([event.event_id,event.kind.id(),
            event.actor_id,event.deflect_pitch,event.target_player_id])).collect::<Vec<_>>(),
        "counters":[enemies.next_actor_id,enemies.next_event_id,enemies.next_projectile_id,
            enemies.next_death_effect_id,enemies.next_native_cell_binding_order,enemies.next_native_registration_order],
        "enemyRng":[json!(enemies.rng_state),player_snapshot::rng_snapshot(&enemies.steering_rng),
            player_snapshot::rng_snapshot(&enemies.locomotion_rng),player_snapshot::rng_snapshot(&enemies.head_facing_rng)],
        "loot":{"rng":player_snapshot::rng_snapshot(&loot.shared_rng),"nextActorId":loot.next_actor_id,
            "nextItemId":loot.next_item_id,"actors":loot_actors,"events":loot_events},
        "bindings":enemies.target_cell_bindings.iter().map(|(id,binding)|
            (id.to_string(),json!({"cellX":binding.cell_x,"cellY":binding.cell_y,"order":binding.order})))
            .collect::<serde_json::Map<_,_>>()})
}

pub fn snapshot(session: &GameSession, id: &str) -> Value {
    let player = player_combat::player_snapshot(&session.participants[id].player);
    let progression = &player["progression"];
    let barrier = session.level_up_barrier.as_ref().map(|barrier| {
        json!({
        "barrierId":barrier.barrier_id,"milestoneExperience":barrier.milestone_experience,
        "milestoneLevel":barrier.milestone_level,"participantIds":barrier.participant_ids,
        "pendingPlayerIds":barrier.pending_player_ids,"runId":barrier.run_id,
        "sourcePlayerId":barrier.source_player_id})
    });
    json!({"tick":session.ticks,"player":player_snapshot::snapshot(&session.participants[id].player,&session.gameplay_rng),
        "combat":player["combat"],"potionClocks":[progression["damageX4TicksRemaining"],
            progression["mindChugTicksRemaining"],progression["poisonImmunityTicksRemaining"],progression["hagathaRuntime"]],
        "runtime":player["runtime"],"character":session_snapshot::character(&session.participants[id]),
        "lighting":session_snapshot::lighting(&session.participants[id].lighting),
        "combatRng":player_snapshot::rng_snapshot(&session.combat_rng),
        "secondary":secondary_state::state_snapshot(&session.secondary),"primary":primary_snapshot(session),
        "barrier":barrier,"run":session_snapshot::run(&session.run),
        "order":{"nextRegistrationOrdinal":{"actor":session.painter_order.next_actor_ordinal,
            "transient":session.painter_order.next_transient_ordinal}},"world":world_snapshot(session, id)})
}

pub fn snapshot_all(session: &GameSession) -> Value {
    let mut shared = snapshot(session, &session.participant_order[0]);
    let mut participants = serde_json::Map::new();
    for id in &session.participant_order {
        let row = snapshot(session, id);
        let mut participant = serde_json::Map::new();
        for key in ["player", "combat", "potionClocks", "runtime", "character", "lighting"] {
            participant.insert(key.into(), row[key].clone());
        }
        let key = if session.boneyard.is_none() { "hub" } else { "score" };
        participant.insert(
            key.into(),
            row["world"][if key == "hub" { "participant" } else { "score" }].clone(),
        );
        participants.insert(id.clone(), Value::Object(participant));
    }
    for key in ["player", "combat", "potionClocks", "runtime", "character", "lighting"] {
        shared.as_object_mut().unwrap().remove(key);
    }
    shared["world"].as_object_mut().unwrap().remove("participant");
    shared["world"].as_object_mut().unwrap().remove("score");
    json!({"ids":session.participant_order,"participants":participants,"shared":shared})
}

// Older oracle families intentionally retain their recorded projection. These
// current-Source entry points also compare the fields added to the public state.
#[allow(dead_code)]
pub fn current_source_snapshot(session: &GameSession, id: &str) -> Value {
    let mut value = snapshot(session, id);
    value["character"]["headingDegrees"] = json!(session.participants[id].wizard.heading_degrees);
    add_current_secondary_fields(&mut value["secondary"], session);
    value
}

#[allow(dead_code)]
pub fn current_source_snapshot_all(session: &GameSession) -> Value {
    let mut value = snapshot_all(session);
    for id in &session.participant_order {
        value["participants"][id.as_str()]["character"]["headingDegrees"] =
            json!(session.participants[id].wizard.heading_degrees);
    }
    add_current_secondary_fields(&mut value["shared"]["secondary"], session);
    value
}

fn add_current_secondary_fields(value: &mut Value, session: &GameSession) {
    let mut camera_displacements = Vec::new();
    for row in &session.secondary.camera_displacements {
        camera_displacements.push(json!({
            "displacement": { "x": row.displacement.x, "y": row.displacement.y },
            "tick": row.tick,
            "worldKey": row.world_key,
        }));
    }
    value["cameraDisplacements"] = Value::Array(camera_displacements);
    value["stoneskinWarp"] = json!(session.secondary.stoneskin_warp);
}
