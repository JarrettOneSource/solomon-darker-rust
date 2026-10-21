use serde_json::{Value, json};
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::world_light::{
    BoneyardLightEnvironment, WorldLightPlayer, WorldLightScene, boneyard_world_light_query,
};
use solomon_sim::enemies::world::EnemyWorld;
use solomon_sim::hub::HubWizard;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::primary::native_primary_skill_profile;
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};
use solomon_sim::spells::casting::NativePlayerWeaponKind;
use solomon_sim::spells::model::*;
use solomon_sim::spells::targeting::*;
use solomon_sim::spells::{PrimarySpellCastAuthority, PrimarySpellInput, PrimarySpellWorldContext};
use solomon_sim::{NativeRng, Vec2};

#[path = "support/website_rows.rs"]
mod website_rows;
use website_rows::compare;

#[path = "support/primary_rows.rs"]
mod primary_rows;
use primary_rows::*;

#[test]
fn elemental_casts_channels_release_and_terrain_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/primary-step-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let element = match case["element"].as_str().unwrap() {
            "air" => WizardElement::Air,
            "earth" => WizardElement::Earth,
            "ether" => WizardElement::Ether,
            "fire" => WizardElement::Fire,
            _ => WizardElement::Water,
        };
        let book = NativeSkillBookState::new(element, WizardDiscipline::Arcane);
        let profile = native_primary_skill_profile(&book, &NativeOffensiveSpellFactors::default());
        let mut wizard = HubWizard::at(Vec2::new(100.0, 200.0));
        wizard.primary_cast.ether_blast_charge = case["charge"].as_f64().unwrap_or(0.0);
        let mut state = SpellState::default();
        let mut rng = NativeRng::new(7);
        let mut mana = case["mana"].as_f64().unwrap();
        let mut order = WorldPainterOrder { next_actor_ordinal: 49, next_transient_ordinal: 3 };
        let blocked = case["blocked"].as_bool().unwrap_or(false);
        let release = case["release"].as_u64().unwrap();
        let targets = [PrimarySpellTarget {
            active: true,
            actor_flags: 2,
            attachment: Vec2::new(0.0, -3.0),
            body_radius: 15.0,
            cell_binding_order: 0,
            heading_degrees: Some(0.0),
            grid_position: None,
            id: "enemy:1".into(),
            kind: PrimarySpellTargetKind::Enemy,
            native_priority: 0,
            pending_remove: false,
            position: Vec2::new(450.0, -150.0),
            query_lane: PrimarySpellQueryLane::Grid,
            registration_order: 0,
        }];
        let can_place = |_: &str, _: Vec2, _: f64| true;
        let can_traverse = |_: solomon_sim::spells::PrimarySpellTraversalActor<'_>,
                            _: Vec2,
                            _: Vec2,
                            _: f64,
                            _: u32| !blocked;
        let obstruction = |start: Vec2, end: Vec2, _: Option<&str>, _: u32| {
            blocked.then_some(Vec2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5))
        };
        let range_endpoint = |start: Vec2, direction: Vec2, _: f64| {
            Vec2::new(start.x + direction.x * 600.0, start.y + direction.y * 600.0)
        };
        let mut frames = case["frames"].as_array().unwrap().iter().peekable();
        for tick in 0..=180 {
            let result = state.step_primary(
                &mut wizard,
                PrimarySpellCastAuthority {
                    weapon_kind: Some(NativePlayerWeaponKind::Staff),
                    alive: true,
                    available_mana: mana,
                    cast_progress_factor: 1.0,
                    eligible: tick < 170,
                    plane_active: false,
                    primary_skill: &profile,
                },
                PrimarySpellInput { aim: Some(Vec2::new(400.0, -100.0)), primary: tick < release },
                &PrimarySpellWorldContext {
                    enhanced_effects: true,
                    flame_lash_view_bounds: None,
                    player_light_registration: None,
                    tick,
                    view_scale: 1.2,
                    owner_id: "player",
                    world_key: "boneyard:oracle",
                    targets: &targets,
                    can_place: &can_place,
                    can_traverse: &can_traverse,
                    obstruction: &obstruction,
                    range_endpoint: &range_endpoint,
                    navigation: None,
                },
                &mut rng,
                Some(&mut order),
            );
            mana -= result.mana_spent;
            if !frames.peek().is_some_and(|frame| frame["tick"] == tick) {
                continue;
            }
            let cast = &wizard.primary_cast;
            let light = boneyard_world_light_query(
                WorldLightScene {
                    bounds: BoneyardBounds::new(0.0, 0.0, 2400.0, 3700.0),
                    camera_bounds: BoneyardBounds::new(0.0, 0.0, 2400.0, 3700.0),
                    lantern_position: None,
                    lantern_light_registration: None,
                },
                "oracle",
                &[WorldLightPlayer {
                    id: "player",
                    position: wizard.position,
                    heading_index: wizard.heading_index,
                    weapon_pulse: cast.weapon_pulse,
                    overlay_effect_phase: 0.25,
                    light_registration: Some(PainterRegistration {
                        manager_lane: WorldManagerLane::Actor,
                        registration_ordinal: 48,
                    }),
                    viewport: Some(Vec2::new(800.0, 600.0)),
                }],
                &EnemyWorld::new("light-oracle", 0),
                tick,
                BoneyardLightEnvironment {
                    primary_spells: Some(&state),
                    secondary_abilities: None,
                },
            );
            let cameras: Vec<_> = light
                .cameras
                .iter()
                .map(|camera| {
                    json!({
                        "x": camera.x, "y": camera.y, "w": camera.width, "h": camera.height,
                    })
                })
                .collect();
            let sources: Vec<_> = light.accepted_sources().map(|source| {
                let mut value = json!({ "intensity": source.intensity, "position": {
                    "x": source.position.x, "y": source.position.y,
                }, "radius": source.radius, "castsDirectionalShadow": source.casts_directional_shadow });
                if let Some(scale) = source.raster_scale { value["rasterScale"] = json!(scale); }
                value
            }).collect();
            let samples: Vec<_> = [
                wizard.position,
                Vec2::new(250.0, 50.0),
                Vec2::new(450.0, -150.0),
                Vec2::new(600.0, 300.0),
                Vec2::new(2000.0, 2500.0),
            ]
            .map(|position| light.scalar_at(position))
            .into();
            let projectiles: Vec<Value> = state.projectiles.iter().map(projectile_row).collect();
            let transients: Vec<Value> = state.transients.iter().map(transient_row).collect();
            let emissions: Vec<Value> = result
                .channel_emissions
                .iter()
                .map(|emission| {
                    json!([
                        emission.damage,
                        point(emission.direction),
                        emission.endpoint.map(point),
                        emission.id,
                        if emission.kind == PrimarySpellChannelKind::Air { "air" } else { "water" },
                        emission.mana_cost,
                        point(emission.origin),
                        point(emission.query_origin),
                        emission.underpowered,
                    ])
                })
                .collect();
            let actual = json!({
                "tick": tick, "mana": mana, "spent": result.mana_spent, "underflow": result.mana_underflow,
                "heading": wizard.heading_index, "cast": [cast.action_tick.unwrap_or(-1.0), point(cast.aim_direction),
                    cast.cast_sequence, cast.channel_active, cast.emission_sequence, cast.ether_blast_charge,
                    cast.ether_blast_charge_cue_sequence, cast.fizzle_sequence, cast.held,
                    cast.one_shot_attack_pose_held, cast.selected_primary_age_ticks, cast.selected_primary_id,
                    cast.target_id, cast.underpowered, cast.weapon_pulse],
                "state": { "nextId": state.next_id, "projectiles": projectiles, "transients": transients },
                "rng": random_state(&rng), "order": [order.next_actor_ordinal, order.next_transient_ordinal], "emissions": emissions,
                "light": { "cameras": cameras, "acceptedSources": sources, "samples": samples },
            });
            compare(&actual, frames.next().unwrap(), &format!("{name}.tick{tick}"));
        }
        assert!(frames.next().is_none());
    }
}
