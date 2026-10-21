use super::{
    common_row, debris_row, point, random, values, vector, website_rows, weld_visual_rows,
};
use serde_json::{Value, json};
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::spells::earth_debris::create_native_weld_ethereal_boulder_breakup_debris_program;
use solomon_sim::spells::model::PrimarySpellProjectilePhase;
use solomon_sim::spells::weld::*;
use solomon_sim::{NativeRng, Vec2};

fn phase(phase: PrimarySpellProjectilePhase) -> &'static str {
    if phase == PrimarySpellProjectilePhase::Held { "held" } else { "flight" }
}

fn persistent_row(actor: &NativeWeldPersistentActorState) -> Value {
    let mut row = common_row(actor.common());
    let fields = match actor {
        NativeWeldPersistentActorState::EtherealBoulder(actor) => json!({
            "assemblyScale":actor.assembly_scale,"damage":actor.damage,"flightTicks":actor.flight_ticks,
            "hitTargetIds":actor.hit_target_ids,"lifetimeTicksRemaining":actor.lifetime_ticks_remaining,
            "maximumScale":actor.maximum_scale,"orientation":actor.orientation,"phase":phase(actor.phase),
            "pulseSequence":actor.pulse_sequence,"quantity":actor.quantity,"remainingDamage":actor.remaining_damage,
            "scale":actor.scale,"shellScale":actor.shell_scale,"speedFactor":actor.speed_factor,
            "toughness":actor.toughness,"velocity":point(actor.velocity),
        }),
        NativeWeldPersistentActorState::Hailstones(actor) => json!({
            "collisionRadius":actor.collision_radius,"damage":actor.damage,"maximumScale":actor.maximum_scale,
            "phase":phase(actor.phase),"pulseSequence":actor.pulse_sequence,"pushback":actor.pushback,
            "releaseAgeTicks":actor.release_age_ticks,"releaseFadeScale":actor.release_fade_scale,
            "rocks":actor.rocks.iter().map(|rock|json!({
                "damageRemaining":rock.damage_remaining,"decay":rock.decay,
                "localPosition":{"x":rock.local_position.x,"y":rock.local_position.y,"z":rock.local_position.z},
                "phase":rock.phase,"rockId":rock.rock_id,"releaseOffset":rock.release_offset.map(point),
                "spriteRecord":rock.sprite_record,"visualScale":rock.visual_scale,
            })).collect::<Vec<_>>(),"scale":actor.scale,"toughness":actor.toughness,"widen":actor.widen,
        }),
        NativeWeldPersistentActorState::MeteorField(actor) => json!({
            "phase":"held","pulseSequence":actor.pulse_sequence,
        }),
    };
    row.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    row["kind"] = json!("weld-persistent");
    row
}

fn fade_row(fade: &NativeWeldFrostFadeActorState) -> Value {
    let mut row = common_row(&fade.common);
    row["kind"] = json!("weld-frost-fade");
    row["scale"] = json!(fade.scale);
    row
}

#[test]
fn held_growth_native_rock_assembly_split_and_flight_keep_source_state_and_rng() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/website/weld-actor-goldens.json")).unwrap();
    for (case_index, case) in fixture["persistentCases"].as_array().unwrap().iter().enumerate() {
        let input = &case["birth"];
        let values = values(input);
        let birth = NativeWeldActorBirth {
            build_id: input["buildId"].as_u64().unwrap() as u32,
            direction: vector(&input["direction"]),
            id: 100,
            origin: vector(&input["origin"]),
            owner_id: "hero",
            tick: 37,
            vector: &values,
            world_key: "boneyard:weld",
        };
        let mut painters =
            WorldPainterOrder { next_actor_ordinal: 500, next_transient_ordinal: 70 };
        let mut actor = create_native_weld_persistent_actor(birth, Some(&mut painters));
        let mut rng = NativeRng::new(17 + case_index as i64);
        for tick in 0..=case["heldTicks"].as_u64().unwrap() {
            let mut update = NativeWeldPersistentUpdate::default();
            if tick > 0 {
                let origin = Vec2::new(
                    birth.origin.x + tick as f64 * 0.125,
                    birth.origin.y - tick as f64 * 0.25,
                );
                let direction =
                    if tick.is_multiple_of(3) { Vec2::new(0.8, 0.6) } else { birth.direction };
                update = update_native_weld_persistent_actor(
                    &mut actor,
                    origin,
                    direction,
                    NativeWeldPersistentUpdateOptions {
                        cast_progress_factor: case["castProgressFactor"].as_f64().unwrap(),
                        enhanced_effects: case["input"]["enhancedEffects"].as_bool().unwrap(),
                        underpowered: case["input"]["weak"].as_bool().unwrap() && tick >= 30,
                    },
                    &mut rng,
                );
            }
            if let Some(expected) =
                case["frames"].as_array().unwrap().iter().find(|frame| frame["tick"] == tick)
            {
                let mut actual =
                    json!({"tick":tick,"actor":persistent_row(&actor),"rng":random(&rng)});
                if tick > 0 {
                    actual["debris"] =
                        json!(update.debris.iter().map(debris_row).collect::<Vec<_>>());
                    actual["hailRockFades"] = json!(update.hail_rock_fades.iter().map(|fade|json!({
                        "position":point(fade.position),"rotationDegrees":fade.rotation_degrees,
                    })).collect::<Vec<_>>());
                    actual["releaseRequested"] = json!(update.release_requested);
                }
                website_rows::compare(
                    &actual,
                    expected,
                    &format!("{} held{tick}", case["input"]["name"]),
                );
                if let Some(sample) = weld_visual_rows::sample(case, tick) {
                    let actor = NativeWeldWorldActor::Persistent(actor.clone());
                    weld_visual_rows::compare(
                        [NativeWeldPresentationState::World(&actor)],
                        sample,
                        "Held Weld compositor",
                    );
                }
            }
        }
        let breakup_scale = if let NativeWeldPersistentActorState::EtherealBoulder(actor) = &actor {
            Some(actor.scale)
        } else {
            None
        };
        let release = release_native_weld_persistent_actor(
            actor,
            500,
            37 + case["heldTicks"].as_u64().unwrap(),
            &mut rng,
            Some(&mut painters),
        );
        let mut rows: Vec<Value> = release.actors.iter().map(persistent_row).collect();
        if let Some(fade) = &release.frost_fade {
            rows.push(fade_row(fade));
        }
        website_rows::compare(
            &json!({"actors":rows,"nextId":release.next_id,"rng":random(&rng)}),
            &case["released"],
            "persistent release",
        );
        let mut actors = release.actors;
        for tick in 1..=1600 {
            if actors.is_empty() {
                break;
            }
            let mut retained = Vec::new();
            let mut terrain_contacts = Vec::new();
            for mut actor in actors {
                let (alive, terrain_contact) =
                    step_native_weld_persistent_actor(&mut actor, &|_, _, _| {
                        tick != case["blockTick"].as_u64().unwrap()
                    });
                if let Some(actor) = terrain_contact {
                    terrain_contacts.push(persistent_row(
                        &NativeWeldPersistentActorState::EtherealBoulder(actor),
                    ));
                }
                if alive {
                    retained.push(actor);
                }
            }
            actors = retained;
            if let Some(expected) =
                case["flight"].as_array().unwrap().iter().find(|frame| frame["tick"] == tick)
            {
                let contact_substeps: Vec<Value> = actors
                    .iter()
                    .map(|actor| {
                        let NativeWeldPersistentActorState::Hailstones(hail) = actor else {
                            return json!([]);
                        };
                        json!(
                            native_weld_hailstone_flight_contact_substeps(hail)
                                .iter()
                                .map(|step| {
                                    let offsets: serde_json::Map<String, Value> = step
                                        .release_offsets
                                        .iter()
                                        .map(|(id, offset)| (id.to_string(), point(*offset)))
                                        .collect();
                                    json!({"origin":point(step.origin),"releaseOffsets":offsets})
                                })
                                .collect::<Vec<_>>()
                        )
                    })
                    .collect();
                let actual = json!({"tick":tick,"actors":actors.iter().map(persistent_row).collect::<Vec<_>>(),
                    "terrainContacts":terrain_contacts,"contactSubsteps":contact_substeps,"rng":random(&rng)});
                website_rows::compare(
                    &actual,
                    expected,
                    &format!("{} flight{tick}", case["input"]["name"]),
                );
                let flight = json!({"visuals":case["flightVisuals"]});
                if let Some(sample) = weld_visual_rows::sample(&flight, tick) {
                    let actors: Vec<_> =
                        actors.iter().cloned().map(NativeWeldWorldActor::Persistent).collect();
                    weld_visual_rows::compare(
                        actors.iter().map(NativeWeldPresentationState::World),
                        sample,
                        "Released Weld compositor",
                    );
                }
            }
        }
        if let Some(scale) = breakup_scale {
            let debris =
                create_native_weld_ethereal_boulder_breakup_debris_program(scale, &mut rng);
            website_rows::compare(
                &json!({"debris":debris.iter().map(debris_row).collect::<Vec<_>>(),"rng":random(&rng)}),
                &case["breakup"],
                "native EBoulder breakup",
            );
        }
        assert_eq!(painters.next_actor_ordinal, case["order"]["actor"].as_u64().unwrap() as usize);
        assert_eq!(painters.next_transient_ordinal, 70);
    }
}
