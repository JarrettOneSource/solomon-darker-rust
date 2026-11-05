//! Stateful source playback requests, replayed over the canonical actor types.

use serde_json::{Value, json};
use solomon_sim::audio::combat::*;
use solomon_sim::audio::{AudioEvent, AudioKind};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::profile::{PlayerStaffAttackOutcome, WizardElement};
use solomon_sim::secondary::*;
use solomon_sim::spells::casting::PlayerPrimaryCastState;
use solomon_sim::spells::fire::{SpawnNativeFireGoodImpOptions, spawn_native_fire_good_imp};
use solomon_sim::spells::model::*;
use solomon_sim::spells::staff::{NativePlayerStaffContactEvent, NativeStaffProcSound};
use solomon_sim::spells::weld::*;
use solomon_sim::{NativeRng, Vec2};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn common(value: &Value) -> PrimarySpellTransientBase {
    PrimarySpellTransientBase {
        id: value["id"].as_u64().unwrap(),
        age_ticks: 0,
        owner_id: value["ownerId"].as_str().unwrap().to_owned(),
        world_key: value["worldKey"].as_str().unwrap().to_owned(),
        light_registration: None,
        painter_registrations: Vec::new(),
    }
}

fn transient(value: &Value) -> PrimarySpellTransient {
    let common = common(value);
    let origin = value.get("origin").or_else(|| value.get("position")).map_or(Vec2::ZERO, point);
    let mut rng = NativeRng::new(1);
    match value["kind"].as_str().unwrap() {
        "water-hail" => PrimarySpellTransient::WaterHail(PrimarySpellWaterHail {
            common,
            birth_tick: 0,
            bounce_progress: 0.0,
            bounce_sound_index: value["bounceSoundIndex"].as_u64().map(|value| value as u32),
            bounce_sound_pitch: value["bounceSoundPitch"].as_f64(),
            bounce_sound_sequence: value["bounceSoundSequence"].as_u64().unwrap(),
            height: 0.0,
            horizontal_velocity: Vec2::ZERO,
            life: 1.0,
            position: origin,
            rotation_degrees: 0.0,
            rotation_step_degrees: 0.0,
            saved_bounce_velocity: 0.0,
            scale: 1.0,
            vertical_velocity: 0.0,
        }),
        "ether-blast" => PrimarySpellTransient::EtherBlast(PrimarySpellEtherBlast {
            common,
            origin,
            birth_tick: 0,
            charges: 1,
            presentation_rng: rng,
        }),
        "fire-impact" => {
            PrimarySpellTransient::FireImpact(PrimarySpellFireImpact { common, origin })
        }
        "fire-explosion" => PrimarySpellTransient::FireExplosion(PrimarySpellFireExplosion {
            common,
            origin,
            presentation: if value["presentation"] == "steam" {
                FireExplosionPresentation::Steam
            } else {
                FireExplosionPresentation::Fire
            },
            sound_pitch: value["soundPitch"].as_f64().unwrap(),
            burn_damage: 0.0,
            damage: 0.0,
            footprint_dimension: 1.0,
            visual_scale: 1.0,
        }),
        "earth-impact" => PrimarySpellTransient::EarthImpact(PrimarySpellEarthImpact {
            common,
            origin,
            birth_tick: 0,
            charge: value["charge"].as_f64().unwrap(),
            enhanced_effects: true,
            lifetime_ticks: 20,
        }),
        "ether-impact" => PrimarySpellTransient::EtherImpact(PrimarySpellEtherImpact {
            common,
            origin,
            birth_tick: 0,
            visual_scale: 1.0,
        }),
        "fire-good-imp" => {
            let mut imp = spawn_native_fire_good_imp(
                SpawnNativeFireGoodImpOptions {
                    id: common.id,
                    owner_id: common.owner_id.clone(),
                    world_key: common.world_key.clone(),
                    position: origin,
                    burn_damage: 0.0,
                    damage: 1.0,
                    lifetime_ticks: 100,
                },
                &mut rng,
            );
            imp.bounce_sound_sequence = value["bounceSoundSequence"].as_u64().unwrap();
            imp.bounce_sound_index = value["bounceSoundIndex"].as_u64().unwrap() as u32;
            imp.bounce_sound_pitch = value["bounceSoundPitch"].as_f64().unwrap();
            imp.contact_sound_sequence = value["contactSoundSequence"].as_u64().unwrap();
            imp.contact_sound_index = value["contactSoundIndex"].as_u64().unwrap() as u32;
            imp.contact_sound_pitch = value["contactSoundPitch"].as_f64().unwrap();
            imp.contact_origin = value.get("contactOrigin").map(point);
            PrimarySpellTransient::FireGoodImp(imp)
        }
        "weld-impact" | "weld-meteor" | "weld-persistent" => {
            let build_id = value["buildId"].as_u64().unwrap_or(1007) as u32;
            let birth = NativeWeldActorBirth {
                id: common.id,
                build_id,
                owner_id: &common.owner_id,
                world_key: &common.world_key,
                tick: 0,
                direction: Vec2::new(0.0, -1.0),
                origin,
                vector: &[1.0; 20],
            };
            let actor = match value["kind"].as_str().unwrap() {
                "weld-impact" => {
                    let mut actor = create_primary_spell_weld_impact(birth, &mut rng);
                    actor.impact_sound_pitch = value["impactSoundPitch"].as_f64();
                    actor.impact_sound_variant =
                        value["impactSoundVariant"].as_u64().map(|value| value as u32);
                    actor.boulder_terminal_charge = value["boulderTerminalCharge"].as_f64();
                    NativeWeldWorldActor::Impact(actor)
                }
                "weld-meteor" => {
                    let spawn = NativeWeldMeteorSpawnProgram {
                        body_scale: 1.0,
                        fall_heading_degrees: 0.0,
                        fall_height: -100.0,
                        fall_step: 1.0,
                        impact_ticks: 20,
                        position: origin,
                        private_seed: 1,
                    };
                    let mut actor = create_native_weld_meteor(
                        birth,
                        spawn,
                        1.0,
                        value["underpowered"].as_bool().unwrap(),
                        None,
                    );
                    actor.position = origin;
                    actor.phase = if value["phase"] == "impact" {
                        NativeWeldMeteorPhase::Impact
                    } else {
                        NativeWeldMeteorPhase::Fall
                    };
                    actor.impact_sound_pitch = value["impactSoundPitch"].as_f64();
                    actor.impact_throw_fire_pitch = value["impactThrowFirePitch"].as_f64();
                    NativeWeldWorldActor::Meteor(actor)
                }
                _ => {
                    let mut actor = create_native_weld_persistent_actor(birth, None);
                    let NativeWeldPersistentActorState::Hailstones(hail) = &mut actor else {
                        panic!("fixture Hailstones")
                    };
                    hail.phase = if value["phase"] == "flight" {
                        PrimarySpellProjectilePhase::Flight
                    } else {
                        PrimarySpellProjectilePhase::Held
                    };
                    NativeWeldWorldActor::Persistent(actor)
                }
            };
            PrimarySpellTransient::Weld(actor)
        }
        kind => panic!("fixture transient {kind} is not mapped"),
    }
}

fn secondary_cue(value: &Value) -> Option<NativeSecondaryAudioCue> {
    let id = value.as_str()?;
    let candidates = [
        NativeSecondaryAudioCue::Dampen,
        NativeSecondaryAudioCue::GolemDie,
        NativeSecondaryAudioCue::GolemProvoke,
        NativeSecondaryAudioCue::LeviathanRoar,
        NativeSecondaryAudioCue::Mindstar,
        NativeSecondaryAudioCue::PlanewalkerOff,
        NativeSecondaryAudioCue::PlanewalkerOn,
        NativeSecondaryAudioCue::PrismaticShock,
        NativeSecondaryAudioCue::QuakeCrackSmall,
        NativeSecondaryAudioCue::QuakeCracks,
        NativeSecondaryAudioCue::SetTrap,
        NativeSecondaryAudioCue::StoneskinOn,
        NativeSecondaryAudioCue::Thunder,
        NativeSecondaryAudioCue::Trap,
        NativeSecondaryAudioCue::MagicMissile,
        NativeSecondaryAudioCue::Fizzle,
        NativeSecondaryAudioCue::CometLoop,
        NativeSecondaryAudioCue::ElectricLoop,
        NativeSecondaryAudioCue::EarthquakeLoop,
        NativeSecondaryAudioCue::LowFireLoop,
        NativeSecondaryAudioCue::PlaneCrossLoop,
        NativeSecondaryAudioCue::RainfallLoop,
        NativeSecondaryAudioCue::SteadyWindLoop,
    ];
    Some(candidates.into_iter().find(|cue| cue.as_str() == id).unwrap())
}

fn primary(value: &Value) -> (SpellState, Vec<NativePlayerStaffContactEvent>) {
    let mut state = SpellState::default();
    let mut contacts = Vec::new();
    for actor in value["transients"].as_array().unwrap() {
        if actor["kind"] == "player-staff-contact" {
            contacts.push(NativePlayerStaffContactEvent {
                common: common(actor),
                origin: point(&actor["origin"]),
                impact_sound_pitches: serde_json::from_value(actor["impactSoundPitches"].clone())
                    .unwrap(),
                pike_break_sound_indexes: serde_json::from_value(
                    actor["pikeBreakSoundIndexes"].clone(),
                )
                .unwrap(),
                proc_sound: Some(NativeStaffProcSound::CriticalHit),
                proc_sound_pitches: serde_json::from_value(actor["procSoundPitches"].clone())
                    .unwrap(),
                swoosh_pitch: actor["swooshPitch"].as_f64().unwrap(),
                outcome: PlayerStaffAttackOutcome::Normal,
                target_ids: Vec::new(),
            });
        } else {
            state.transients.push(transient(actor));
        }
    }
    for spell in value["projectiles"].as_array().unwrap() {
        let common = PrimarySpellProjectileBase {
            id: spell["id"].as_u64().unwrap(),
            age_ticks: 0,
            charge: spell["charge"].as_f64().unwrap(),
            damage: 1.0,
            direction: Vec2::new(0.0, -1.0),
            flight_ticks: 0,
            registration: PainterRegistration {
                manager_lane: WorldManagerLane::Actor,
                registration_ordinal: 0,
            },
            owner_id: spell["ownerId"].as_str().unwrap().to_owned(),
            world_key: spell["worldKey"].as_str().unwrap().to_owned(),
            phase: if spell["phase"] == "held" {
                PrimarySpellProjectilePhase::Held
            } else {
                PrimarySpellProjectilePhase::Flight
            },
            position: point(&spell["position"]),
            velocity: Vec2::ZERO,
        };
        state.projectiles.push(PrimarySpellProjectile::Earth(PrimarySpellEarthProjectile {
            common,
            assembly_charge: 0.5,
            hit_target_ids: Vec::new(),
            maximum_charge: 1.0,
            orientation: [0.0; 9],
            remaining_damage: 1.0,
            shell_charge: 0.5,
            toughness: 1.0,
        }));
    }
    (state, contacts)
}

fn secondary(value: &Value) -> NativeSecondarySimulationState {
    let mut state = NativeSecondarySimulationState::new(1);
    state.next_event_id = value["nextEventId"].as_u64().unwrap();
    for (id, input) in value["players"].as_object().unwrap() {
        state.players.insert(
            id.clone(),
            NativeSecondaryPlayerState {
                planewalker_ticks_remaining: input["planewalkerTicksRemaining"].as_u64().unwrap(),
                ..NativeSecondaryPlayerState::default()
            },
        );
    }
    for input in value["actors"].as_array().unwrap() {
        let kind = NativeSecondaryActorKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == input["kind"].as_str().unwrap())
            .unwrap();
        let mut actor = NativeSecondaryActorState::new(
            input["id"].as_u64().unwrap(),
            kind,
            input["ownerId"].as_str().unwrap().to_owned(),
            input["worldKey"].as_str().unwrap().to_owned(),
            None,
        );
        actor.position = point(&input["position"]);
        actor.phase = input["phase"].as_f64().unwrap_or(0.0);
        actor.age_ticks = input["ageTicks"].as_u64().unwrap_or(0);
        state.actors.push(actor);
    }
    for input in value["events"].as_array().unwrap() {
        let mut event = NativeSecondaryEventState::new(
            NativeSecondaryEventKind::Cast,
            "local".to_owned(),
            input["worldKey"].as_str().unwrap().to_owned(),
            0,
        );
        event.event_id = input["eventId"].as_u64().unwrap();
        event.cue = secondary_cue(&input["cue"]);
        event.gain = input["gain"].as_f64().unwrap();
        event.pitch = input["pitch"].as_f64().unwrap();
        event.position = point(&input["position"]);
        state.events.push(event);
    }
    state
}

fn output_json(output: &CombatAudioOutput) -> Value {
    let events: Vec<_> = output
        .events
        .iter()
        .filter_map(|event| match event {
            AudioEvent::Play(cue, options) => Some(json!({ "cue": cue.as_str(),
            "kind": if cue.definition().kind == AudioKind::Stream { "stream" } else { "sound" },
            "playbackRate": options.playback_rate, "volume": options.volume })),
            _ => None,
        })
        .collect();
    let loops: Vec<_> = output.loops.iter().map(|command| match command {
        CombatAudioLoopCommand::Start(request) => json!({ "action": "start", "cue": request.cue.as_str(),
            "owner": request.owner, "playbackRate": request.options.playback_rate, "volume": request.options.volume }),
        CombatAudioLoopCommand::Stop { cue, owner } => json!({ "action": "stop", "cue": cue.as_str(), "owner": owner }),
    }).collect();
    json!({ "events": events, "loops": loops })
}

fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} keys");
            for (key, value) in expected {
                compare(&actual[key], value, &format!("{path}.{key}"));
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} count: {actual:?} != {expected:?}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                compare(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}")
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn accepted_casts_actor_edges_and_owned_loops_match_the_source_synchronizer() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/combat-audio-goldens.json")).unwrap();
    for scenario in fixture["scenarios"].as_array().unwrap() {
        let mut cursor = None;
        for (index, record) in scenario["frames"].as_array().unwrap().iter().enumerate() {
            let snapshot = &record["snapshot"];
            let (primary, staff_contacts) = primary(&snapshot["primarySpells"]);
            let secondary = secondary(&snapshot["secondaryAbilities"]);
            let player = &snapshot["players"]["local"];
            let input = &player["primaryCast"];
            let cast = PlayerPrimaryCastState {
                cast_sequence: input["castSequence"].as_u64().unwrap(),
                emission_sequence: input["emissionSequence"].as_u64().unwrap(),
                fizzle_sequence: input["fizzleSequence"].as_u64().unwrap(),
                ether_blast_charge_cue_sequence: input["etherBlastChargeCueSequence"]
                    .as_u64()
                    .unwrap(),
                channel_active: input["channelActive"].as_bool().unwrap(),
                underpowered: input["underpowered"].as_bool().unwrap(),
                last_weld_playback_rate: input["lastWeldPlaybackRate"].as_f64(),
                last_weld_sound_variant: input["lastWeldSoundVariant"]
                    .as_u64()
                    .map(|value| value as u32),
                ..PlayerPrimaryCastState::default()
            };
            let players = [CombatAudioPlayer {
                id: "local",
                world_key: "boneyard:audio-run",
                position: point(&player["position"]),
                element: WizardElement::ALL
                    .into_iter()
                    .find(|element| {
                        element.as_str() == player["config"]["element"].as_str().unwrap()
                    })
                    .unwrap(),
                selected_primary_skill_id: player["progression"]["selectedPrimarySkillId"]
                    .as_u64()
                    .unwrap() as u32,
                weld_build_id: player["progression"]["weldBuildId"]
                    .as_u64()
                    .map(|value| value as u32),
                cast: &cast,
                footstep_tick: 0,
            }];
            let frame = CombatAudioFrame {
                tick: index as u64,
                run_id: Some("audio-run"),
                listener_id: "local",
                players: &players,
                primary: &primary,
                secondary: &secondary,
                staff_contacts: &staff_contacts,
                enemies: None,
                enemy_events: &[],
                loot_events: &[],
                encounter: None,
                point_gain: &|_| 1.0,
                hit_gain: &|_| 1.0,
            };
            let cursor = cursor.get_or_insert_with(|| CombatAudioCursor::new(&frame));
            let output = cursor.update(&frame);
            compare(
                &output_json(&output),
                &record["expected"],
                &format!("{} frame {index}", scenario["name"]),
            );
        }
        let output = cursor.unwrap().destroy();
        compare(&output_json(&output)["loops"], &scenario["destroy"]["loops"], "destroy loops");
    }
    for record in fixture["weld"].as_array().unwrap() {
        let sounds: Vec<_> = native_weld_cast_sound_cues(
            record["buildId"].as_u64().unwrap() as u32,
            record["variant"].as_u64().map(|value| value as u32),
        )
        .iter()
        .map(|cue| cue.as_str())
        .collect();
        assert_eq!(json!(sounds), record["sounds"]);
        let loops: Vec<_> = native_weld_loop_cues(record["buildId"].as_u64().unwrap() as u32)
            .iter()
            .map(|cue| cue.as_str())
            .collect();
        assert_eq!(json!(loops), record["loops"]);
    }
    for record in fixture["accepted"].as_array().unwrap() {
        let output =
            native_air_water_accepted_cast_audio(record["skillId"].as_u64().unwrap() as u32);
        let actual = output.map_or(Value::Null, |events| {
            let mut requests = output_json(&CombatAudioOutput {
                events,
                ..CombatAudioOutput::default()
            })["events"]
                .clone();
            for request in requests.as_array_mut().unwrap() {
                request.as_object_mut().unwrap().remove("volume");
            }
            requests
        });
        compare(&actual, &record["expected"], "accepted cast");
    }
}
