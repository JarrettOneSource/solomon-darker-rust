//! Public cast and owner phases preserve the source Air/Water queue and combat RNG.
use serde_json::{Value, json};
use solomon_sim::hub::HubWizard;
use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::primary::{NativePrimarySkillProfile, native_primary_skill_profile};
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};
use solomon_sim::spells::air_water::*;
use solomon_sim::spells::casting::NativePlayerWeaponKind;
use solomon_sim::spells::model::*;
use solomon_sim::spells::{PrimarySpellCastAuthority, PrimarySpellInput, PrimarySpellWorldContext};
use solomon_sim::{NativeRng, Vec2};
#[path = "support/website_rows.rs"]
mod website_rows;

fn point(value: Vec2) -> [f64; 2] {
    [value.x, value.y]
}
fn actor_row(actor: &PrimarySpellTransient) -> Value {
    let common = actor.common();
    let (kind, birth, fields) = match actor {
        PrimarySpellTransient::AirHurricane(actor) => (
            "air-hurricane",
            Some(actor.birth_tick),
            json!({
                "charge":actor.charge,"contactCharge":actor.contact_charge,"damageMaximum":actor.damage_maximum,
                "damageMinimum":actor.damage_minimum,"enhancedEffects":actor.enhanced_effects,"phaseDegrees":actor.phase_degrees,
                "position":point(actor.position),"lanes":actor.lanes.map(|lane|[lane.angle_degrees,lane.angular_velocity_degrees,lane.radius,lane.vertical_offset]),
            }),
        ),
        PrimarySpellTransient::WaterAura(actor) => (
            "water-aura",
            Some(actor.birth_tick),
            json!({
                "alphaDecay":actor.alpha_decay,"durationTicks":actor.duration_ticks,"initialRotationDegrees":actor.initial_rotation_degrees,
                "rotationStepDegrees":actor.rotation_step_degrees,"origin":point(actor.origin),
            }),
        ),
        PrimarySpellTransient::WaterHail(actor) => (
            "water-hail",
            Some(actor.birth_tick),
            json!({
                "bounceProgress":actor.bounce_progress,"bounceSoundIndex":actor.bounce_sound_index,"bounceSoundPitch":actor.bounce_sound_pitch,
                "bounceSoundSequence":actor.bounce_sound_sequence,"height":actor.height,"life":actor.life,"rotationDegrees":actor.rotation_degrees,
                "rotationStepDegrees":actor.rotation_step_degrees,"savedBounceVelocity":actor.saved_bounce_velocity,"scale":actor.scale,
                "verticalVelocity":actor.vertical_velocity,"position":point(actor.position),"horizontalVelocity":point(actor.horizontal_velocity),
            }),
        ),
        PrimarySpellTransient::Water(actor) => {
            ("water", None, json!({"origin":point(actor.origin)}))
        }
        _ => panic!("unexpected actor in Air/Water source journey"),
    };
    json!({"age":common.age_ticks,"birth":birth,"id":common.id,"kind":kind,"owner":common.owner_id,"world":common.world_key,"light":null,
        "painters":common.painter_registrations.iter().map(|r|json!([if r.manager_lane==WorldManagerLane::Actor {"actor"}else{"transient"},r.registration_ordinal])).collect::<Vec<_>>(),"fields":fields})
}
fn hurricane_plan(actor: &PrimarySpellAirHurricane) -> Value {
    json!(
        native_hurricane_visual_plan(actor)
            .iter()
            .map(|draw| json!([
                draw.alpha,
                draw.additive,
                point(draw.position),
                match draw.role {
                    NativeHurricaneSpriteRole::Core => "core",
                    NativeHurricaneSpriteRole::Lane => "lane",
                    NativeHurricaneSpriteRole::LaneCopy => "lane-copy",
                },
                draw.rotation_radians,
                point(draw.scale),
                draw.tint,
            ]))
            .collect::<Vec<_>>()
    )
}
fn visual_row(actor: &PrimarySpellTransient) -> Value {
    match actor {
        PrimarySpellTransient::AirHurricane(actor) => {
            let mut reduced = actor.clone();
            reduced.enhanced_effects = false;
            json!({"full":hurricane_plan(actor),"reduced":hurricane_plan(&reduced)})
        }
        PrimarySpellTransient::WaterAura(actor) => {
            let draw = native_water_aura_visual_plan(actor);
            json!({"alpha":draw.alpha,"rotationRadians":draw.rotation_radians,"scale":draw.scale,"tint":draw.tint})
        }
        PrimarySpellTransient::WaterHail(actor) => {
            let draw = native_hail_visual_plan(actor);
            json!({"alpha":draw.alpha,"offsetY":draw.offset_y,"rotationRadians":draw.rotation_radians,"scale":draw.scale})
        }
        _ => Value::Null,
    }
}
#[test]
fn owner_synchronization_hail_births_bounces_and_aura_finalization_preserve_source_order() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/air-water-player-visual-goldens.json"))
            .unwrap();
    let book = NativeSkillBookState::new(WizardElement::Water, WizardDiscipline::Arcane);
    let NativePrimarySkillProfile::Water(mut water) =
        native_primary_skill_profile(&book, &NativeOffensiveSpellFactors::default())
    else {
        unreachable!()
    };
    water.aura_radius_scale = 1.0;
    water.hail_threshold = 125;
    water.hail_damage_minimum = 0.25;
    water.hail_damage_maximum = 1.5;
    water.widen_half_degrees = 10.0;
    let profile = NativePrimarySkillProfile::Water(water);
    for case in fixture["cases"].as_array().unwrap() {
        let mode = case["mode"].as_str().unwrap();
        let mut wizard = HubWizard::at(Vec2::new(150.0, 250.0));
        let mut spells = SpellState::default();
        let mut rng = NativeRng::new(931);
        let mut order = WorldPainterOrder { next_actor_ordinal: 50, next_transient_ordinal: 3 };
        let mut mana = if mode == "underpowered-water" { 0.001 } else { 1000.0 };
        for tick in 0..=180 {
            let world_key = if tick < 80 { "boneyard:weather" } else { "boneyard:next" };
            let result = spells.step_primary(
                &mut wizard,
                PrimarySpellCastAuthority {
                    weapon_kind: Some(NativePlayerWeaponKind::Staff),
                    alive: true,
                    available_mana: mana,
                    cast_progress_factor: 1.0,
                    eligible: true,
                    plane_active: false,
                    primary_skill: &profile,
                },
                PrimarySpellInput {
                    aim: Some(Vec2::new(700.0, 250.0)),
                    primary: mode != "hurricane-owner-world-switch" && tick < 40,
                },
                &PrimarySpellWorldContext {
                    enhanced_effects: true,
                    flame_lash_view_bounds: None,
                    player_light_registration: None,
                    tick,
                    view_scale: 1.2,
                    owner_id: "hero",
                    world_key,
                    targets: &[],
                    can_place: &|_, _, _| true,
                    can_traverse: &|_, _, _, _, _| true,
                    obstruction: &|_, _, _, _| None,
                    range_endpoint: &|start, direction, _| {
                        Vec2::new(start.x + direction.x * 600.0, start.y + direction.y * 600.0)
                    },
                    navigation: None,
                },
                &mut rng,
                Some(&mut order),
            );
            mana -= result.mana_spent;
            let mut owners = Vec::new();
            if tick < 150 {
                owners.push(AirWaterPlayerVisualOwner {
                    hurricane_contact_charge: if mode.starts_with("hurricane") {
                        if tick % 20 == 0 { 0.0 } else { 0.25 }
                    } else {
                        0.0
                    },
                    hurricane_charge: if mode.starts_with("hurricane") { 0.5 } else { 0.0 },
                    hurricane_damage_maximum: 4.0,
                    hurricane_damage_minimum: 2.0,
                    owner_id: "hero".into(),
                    position: Vec2::new(150.0 + tick as f64 * 0.5, 250.0 - tick as f64 * 0.25),
                    world_key: world_key.into(),
                });
            }
            if mode.starts_with("hurricane") && tick < 100 {
                owners.push(AirWaterPlayerVisualOwner {
                    hurricane_contact_charge: 1.0,
                    hurricane_charge: 1.0,
                    hurricane_damage_maximum: 6.0,
                    hurricane_damage_minimum: 3.0,
                    owner_id: "other".into(),
                    position: Vec2::new(300.0, 300.0),
                    world_key: "hub:courtyard".into(),
                });
            }
            synchronize_air_water_player_visual_actors(
                &mut spells,
                &owners,
                tick,
                &mut rng,
                &result.channel_emissions,
                Some(&mut order),
                true,
            );
            finalize_air_water_player_visual_actors(
                &mut spells,
                &result.channel_emissions,
                tick,
                &mut rng,
                &mut order,
            );
            let Some(expected) =
                case["frames"].as_array().unwrap().iter().find(|frame| frame["tick"] == tick)
            else {
                continue;
            };
            let actual = json!({"tick":tick,"mana":mana,"nextId":spells.next_id,"actors":spells.transients.iter().map(actor_row).collect::<Vec<_>>(),
                "visuals":spells.transients.iter().map(visual_row).collect::<Vec<_>>(),"rng":{"indexA":rng.index_a,"indexB":rng.index_b,"words":rng.words.as_slice()},
                "order":{"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}});
            website_rows::compare(&actual, expected, &format!("{mode} tick{tick}"));
        }
    }
}
