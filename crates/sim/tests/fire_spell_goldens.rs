use serde_json::{Value, json};
use solomon_sim::enemies::path::NativeEnemyPathState;
use solomon_sim::profile::primary::NativeSpentEmber;
use solomon_sim::spells::fire::*;
use solomon_sim::spells::model::*;
use solomon_sim::spells::targeting::{
    PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
};
use solomon_sim::{NativeRng, Vec2};

#[path = "support/website_rows.rs"]
mod website_rows;
use website_rows::compare;

fn point(position: Vec2) -> Value {
    json!({ "x": position.x, "y": position.y })
}

fn random_state(rng: &NativeRng) -> Value {
    json!({ "indexA": rng.index_a, "indexB": rng.index_b, "words": rng.words.as_slice() })
}

fn spent_ember(value: NativeSpentEmber) -> Value {
    match value {
        NativeSpentEmber::None => json!({ "kind": "none" }),
        NativeSpentEmber::Immolate { damage } => json!({ "kind": "immolate", "damage": damage }),
        NativeSpentEmber::Imp { damage, lifetime_ticks } => {
            json!({ "kind": "imp", "damage": damage, "lifetimeTicks": lifetime_ticks })
        }
    }
}

fn patch_row(patch: &PrimarySpellFirePatch) -> Value {
    let native_type = match patch.native_type {
        FirePatchType::Fire => "fire",
        FirePatchType::GoodGuy => "goodguy",
        FirePatchType::Moving => "moving",
    };
    let registrations: Vec<Value> = patch.common.painter_registrations.iter().map(|registration| {
        json!({ "managerLane": "actor", "registrationOrdinal": registration.registration_ordinal })
    }).collect();
    json!({
        "ageTicks": patch.common.age_ticks, "atlasPhase": patch.atlas_phase,
        "atlasPhaseStep": patch.atlas_phase_step, "burnDamage": patch.burn_damage,
        "damage": patch.damage, "drawAlpha": patch.draw_alpha, "fadeAlpha": patch.fade_alpha,
        "id": patch.common.id, "kind": "fire-patch", "life": patch.life,
        "nativeType": native_type, "ownerId": patch.common.owner_id,
        "painterRegistrations": registrations, "position": point(patch.position),
        "scale": patch.scale, "horizontalSign": patch.horizontal_sign,
        "supplementalContact": patch.supplemental_contact, "velocity": point(patch.velocity),
        "velocityMultiplier": point(patch.velocity_multiplier), "worldKey": patch.common.world_key,
    })
}

fn radial_contact(contact: &FireRadialContact, kind: &str) -> Value {
    json!({
        "amount": contact.amount, "burnDamage": contact.burn_damage, "kind": kind,
        "ownerId": contact.owner_id, "position": point(contact.position), "radius": contact.radius,
        "spellId": contact.spell_id, "worldKey": contact.world_key,
    })
}

fn target_contact(contact: &FireTargetContact) -> Value {
    json!({
        "amount": contact.amount, "kind": "fire-good-imp", "ownerId": contact.owner_id,
        "spellId": contact.spell_id, "targetId": contact.target_id, "worldKey": contact.world_key,
    })
}

fn ember_row(ember: &PrimarySpellFireEmber) -> Value {
    json!({
        "ageTicks": ember.common.age_ticks, "burnDamage": ember.burn_damage,
        "contactCadence": ember.contact_cadence, "contactDue": ember.contact_due,
        "damage": ember.damage, "height": ember.height,
        "horizontalVelocity": point(ember.horizontal_velocity), "id": ember.common.id,
        "life": ember.life, "ownerId": ember.common.owner_id, "phase": ember.phase,
        "position": point(ember.position), "spentEmber": spent_ember(ember.spent_ember),
        "verticalVelocity": ember.vertical_velocity, "worldKey": ember.common.world_key,
    })
}

fn explosion_row(explosion: &NativeFireExplosionState) -> Value {
    json!({
        "burnDamage": explosion.burn_damage, "damage": explosion.damage,
        "footprintDimension": explosion.footprint_dimension, "origin": point(explosion.origin),
        "ownerId": explosion.owner_id, "presentation": "fire",
        "visualScale": explosion.visual_scale, "worldKey": explosion.world_key,
    })
}

fn retirement_row(retirement: &NativeFireEmberRetirement) -> Value {
    match retirement {
        NativeFireEmberRetirement::None => json!({ "kind": "none" }),
        NativeFireEmberRetirement::Immolate(explosion) => {
            json!({ "kind": "immolate", "explosion": explosion_row(explosion) })
        }
        NativeFireEmberRetirement::Imp {
            burn_damage,
            damage,
            lifetime_ticks,
            owner_id,
            position,
            world_key,
        } => json!({
            "kind": "imp", "burnDamage": burn_damage, "damage": damage,
            "lifetimeTicks": lifetime_ticks, "ownerId": owner_id,
            "position": point(*position), "worldKey": world_key,
        }),
    }
}

fn path_row(path: &NativeEnemyPathState) -> Value {
    let waypoints = path.route_waypoints.map(|points| [point(points[0]), point(points[1])]);
    json!({
        "baseTurnRate": path.base_turn_rate, "flankAngleDeg": path.flank_angle_deg,
        "flankRadius": path.flank_radius, "flankTicksRemaining": path.flank_ticks_remaining,
        "reorientationTicksRemaining": path.reorientation_ticks_remaining,
        "routePreviousVector": path.route_previous_vector.map(point),
        "routeRefreshTicksRemaining": path.route_refresh_ticks_remaining,
        "routeTicksRemaining": path.route_ticks_remaining, "routeWaypointIndex": path.route_waypoint_index,
        "routeWaypoints": waypoints, "speedFactor": path.speed_factor,
        "stalledMovementTicks": path.stalled_movement_ticks, "turnFactor": path.turn_factor,
        "wanderHeadingDeg": path.wander_heading_deg,
    })
}

fn imp_row(imp: &PrimarySpellFireGoodImp) -> Value {
    json!({
        "ageTicks": imp.common.age_ticks, "bodyRotationDeg": imp.body_rotation_degrees,
        "bodyScale": imp.body_scale, "bodyVariant": imp.body_variant,
        "bounceSoundIndex": imp.bounce_sound_index, "bounceSoundPitch": imp.bounce_sound_pitch,
        "bounceSoundSequence": imp.bounce_sound_sequence, "burnDamage": imp.burn_damage,
        "collisionRadius": imp.collision_radius, "contactAgeTicks": imp.contact_age_ticks,
        "contactOrigin": imp.contact_origin.map(point), "contactScale": imp.contact_scale,
        "contactSoundIndex": imp.contact_sound_index, "contactSoundPitch": imp.contact_sound_pitch,
        "contactSoundSequence": imp.contact_sound_sequence, "damage": imp.damage,
        "effectAlpha": imp.effect_alpha, "effectPhase": imp.effect_phase, "flightSpeed": imp.flight_speed,
        "headingDegrees": imp.heading_degrees, "id": imp.common.id, "lightGlow": imp.light_glow,
        "nextTargetRefreshTick": imp.next_target_refresh_tick, "ownerId": imp.common.owner_id,
        "path": path_row(&imp.path), "position": point(imp.position), "remainingTicks": imp.remaining_ticks,
        "targetId": imp.target_id, "verticalOffset": imp.vertical_offset,
        "verticalVelocity": imp.vertical_velocity, "worldKey": imp.common.world_key,
    })
}

fn targets() -> Vec<PrimarySpellTarget> {
    let mut targets = Vec::new();
    for (index, position) in
        [Vec2::new(100.0, 0.0), Vec2::new(150.0, 120.0)].into_iter().enumerate()
    {
        targets.push(PrimarySpellTarget {
            active: true,
            actor_flags: 2,
            attachment: Vec2::ZERO,
            body_radius: 15.0,
            cell_binding_order: index,
            heading_degrees: Some(0.0),
            grid_position: None,
            id: if index == 0 { "enemy:a".into() } else { "enemy:b".into() },
            kind: PrimarySpellTargetKind::Enemy,
            native_priority: 0,
            pending_remove: false,
            position,
            query_lane: PrimarySpellQueryLane::Grid,
            registration_order: index,
        });
    }
    targets
}

#[test]
fn fire_native_lifecycles_and_random_streams_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/fire-spell-goldens.json")).unwrap();
    for case in fixture["patches"].as_array().unwrap() {
        let moving = case["nativeType"] == "moving";
        let native_type = match case["nativeType"].as_str().unwrap() {
            "fire" => FirePatchType::Fire,
            "goodguy" => FirePatchType::GoodGuy,
            _ => FirePatchType::Moving,
        };
        let mut rng = NativeRng::new(7);
        let mut patch = Some(spawn_native_fire_patch(
            CreateNativeFirePatchOptions {
                atlas_phase_step: None,
                burn_damage: 10.0,
                damage: 8.0,
                draw_alpha: None,
                fade_alpha: None,
                id: 1,
                life: moving.then_some(0.03),
                native_type,
                owner_id: "owner".into(),
                painter_registration: None,
                position: Vec2::new(123.1, -456.2),
                scale: moving.then_some(0.8),
                supplemental_contact: moving,
                velocity: moving.then_some(Vec2::new(1.25, -0.5)),
                velocity_multiplier: moving.then_some(Vec2::new(0.95, 0.75)),
                world_key: "boneyard".into(),
            },
            &mut rng,
        ));
        let mut tick = 0;
        let mut contact = None;
        for frame in case["frames"].as_array().unwrap() {
            let expected_tick = frame["tick"].as_u64().unwrap();
            while tick < expected_tick {
                tick += 1;
                let (retained, next_contact) =
                    step_native_fire_patch(patch.as_mut().unwrap(), tick);
                contact = next_contact;
                if !retained {
                    patch = None;
                }
            }
            compare(
                &patch.as_ref().map(patch_row).unwrap_or(Value::Null),
                &frame["patch"],
                "patch",
            );
            compare(
                &contact
                    .as_ref()
                    .map(|value| radial_contact(value, "fire-patch"))
                    .unwrap_or(Value::Null),
                &frame["contact"],
                "patch.contact",
            );
        }
        compare(&random_state(&rng), &case["rng"], "patch.rng");
    }

    for case in fixture["detonations"].as_array().unwrap() {
        let spent = match case["kind"].as_str().unwrap() {
            "immolate" => NativeSpentEmber::Immolate { damage: 20.0 },
            "imp" => NativeSpentEmber::Imp { damage: 20.0, lifetime_ticks: 300 },
            _ => NativeSpentEmber::None,
        };
        let mut rng = NativeRng::new(900);
        let born = create_native_fire_detonation(
            10,
            NativeFireProjectilePayload {
                burn_damage: 10.0,
                ember_damage: 8.0,
                ember_fragments: 4,
                explode_damage: 12.0,
                explode_radius: 15.0,
                private_seed: 123456,
                spent_ember: spent,
            },
            Vec2::new(100.0, 200.0),
            "owner",
            "boneyard",
            &mut rng,
        );
        let contacts: Vec<Value> =
            born.contacts.iter().map(|value| radial_contact(value, "fire-ember")).collect();
        let embers: Vec<Value> = born.embers.iter().map(ember_row).collect();
        compare(
            &json!({
                "contacts": contacts, "embers": embers,
                "explosion": born.explosion.as_ref().map(explosion_row), "nextId": born.next_id,
                "rng": random_state(&rng), "soundPitch": born.sound_pitch,
            }),
            &case["born"],
            "detonation",
        );
        let mut ember = Some(born.embers[0].clone());
        let mut retirement = NativeFireEmberRetirement::None;
        let mut tick = 0;
        for frame in case["frames"].as_array().unwrap() {
            while tick < frame["tick"].as_u64().unwrap() {
                tick += 1;
                let step = step_native_fire_ember(ember.as_mut().unwrap());
                if !step.retained {
                    ember = None;
                }
                retirement = step.retirement;
            }
            compare(
                &ember.as_ref().map(ember_row).unwrap_or(Value::Null),
                &frame["ember"],
                "ember",
            );
            compare(&retirement_row(&retirement), &frame["retirement"], "retirement");
        }
    }

    for case in fixture["imps"].as_array().unwrap() {
        let mode = case["mode"].as_str().unwrap();
        let mut rng = NativeRng::new(7);
        let mut imp = Some(spawn_native_fire_good_imp(
            SpawnNativeFireGoodImpOptions {
                burn_damage: 10.0,
                damage: 12.0,
                id: 51,
                lifetime_ticks: case["options"]["lifetimeTicks"].as_u64().unwrap(),
                owner_id: "owner".into(),
                position: Vec2::ZERO,
                world_key: "boneyard".into(),
            },
            &mut rng,
        ));
        let mut actors = if mode == "empty" { Vec::new() } else { targets() };
        let can_occupy = |_: Vec2| mode != "blocked";
        let mut tick = 0;
        let mut contact = None;
        let mut release_fire = false;
        let mut release_position = Vec2::ZERO;
        let mut all_contacts = Vec::new();
        for frame in case["frames"].as_array().unwrap() {
            while tick < frame["tick"].as_u64().unwrap() {
                tick += 1;
                if tick >= 60 && !actors.is_empty() {
                    actors[0].active = false;
                }
                let step = step_native_fire_good_imp(
                    imp.as_mut().unwrap(),
                    NativeFireGoodImpStepContext {
                        can_occupy: &can_occupy,
                        navigation: None,
                        targets: &actors,
                    },
                    &mut rng,
                );
                release_position = imp.as_ref().unwrap().position;
                if !step.retained {
                    imp = None;
                }
                contact = step.contact;
                release_fire = !step.retained;
                if let Some(value) = &contact {
                    let mut row = target_contact(value);
                    row["tick"] = json!(tick);
                    all_contacts.push(row);
                }
            }
            compare(&imp.as_ref().map(imp_row).unwrap_or(Value::Null), &frame["goodImp"], "imp");
            compare(&random_state(&rng), &frame["rng"], "imp.rng");
            compare(
                &contact.as_ref().map(target_contact).unwrap_or(Value::Null),
                &frame["contact"],
                "imp.contact",
            );
            assert_eq!(release_fire, frame["releaseFire"]);
            compare(&point(release_position), &frame["releasePosition"], "imp.releasePosition");
        }
        compare(&json!(all_contacts), &case["contacts"], "imp.contacts");
    }

    for case in fixture["presentationCases"].as_array().unwrap() {
        let id = case["id"].as_u64().unwrap();
        compare(
            &json!({
                "id": id, "fade": native_fire_particle_fade_step(id, true),
                "lifetime": native_fire_particle_lifetime_ticks(id, true), "variant": native_fire_particle_variant(id),
                "pitch": native_fire_impact_pitch(id), "signed": native_fire_presentation_signed_random(id, 7, 3, 0.5),
            }),
            case,
            "presentation",
        );
    }
}
