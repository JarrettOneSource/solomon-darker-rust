use serde_json::{Value, json};
use solomon_sim::hub::HubWizard;
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::spells::casting::NativePlayerWeaponKind;
use solomon_sim::spells::model::SpellState;
use solomon_sim::spells::targeting::{
    PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
};
use solomon_sim::spells::{PrimarySpellCastAuthority, PrimarySpellInput, PrimarySpellWorldContext};
use solomon_sim::{NativeRng, Vec2};

#[path = "weld_step_goldens/rows.rs"]
mod rows;
#[path = "support/website_rows.rs"]
mod website_rows;

#[path = "support/weld_profile.rs"]
mod weld_profile;
use weld_profile::profile;

#[test]
fn all_ten_weld_casts_preserve_native_mana_clocks_queues_and_world_epochs() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/weld-step-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let profile = profile(&case["profile"]);
        let blocked = case["blocked"].as_bool().unwrap();
        let progress = case["progress"].as_f64().unwrap();
        let release = case["release"].as_u64().unwrap();
        let mut mana = case["mana"].as_f64().unwrap();
        let mut wizard = HubWizard::at(Vec2::new(100.0, 200.0));
        let mut state = SpellState::default();
        let mut rng = NativeRng::new(731);
        let mut order = WorldPainterOrder { next_actor_ordinal: 49, next_transient_ordinal: 3 };
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
            position: Vec2::new(450.0, 200.0),
            query_lane: PrimarySpellQueryLane::Grid,
            registration_order: 0,
        }];
        let obstruction = |start: Vec2, end: Vec2, _: Option<&str>, _: u32| {
            blocked.then_some(Vec2::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0))
        };
        for tick in 0..=320 {
            let result = state.step_primary(
                &mut wizard,
                PrimarySpellCastAuthority {
                    weapon_kind: Some(NativePlayerWeaponKind::Staff),
                    alive: true,
                    available_mana: mana,
                    cast_progress_factor: progress,
                    eligible: tick < 260,
                    plane_active: false,
                    primary_skill: &profile,
                },
                PrimarySpellInput { aim: Some(Vec2::new(700.0, 200.0)), primary: tick < release },
                &PrimarySpellWorldContext {
                    enhanced_effects: true,
                    flame_lash_view_bounds: None,
                    player_light_registration: None,
                    tick,
                    view_scale: 1.2,
                    owner_id: "hero",
                    world_key: "boneyard:weld",
                    targets: &targets,
                    can_place: &|_, _, _| true,
                    can_traverse: &|_, _, _, _, _| !blocked,
                    obstruction: &obstruction,
                    range_endpoint: &|start, direction, _| {
                        Vec2::new(start.x + direction.x * 600.0, start.y + direction.y * 600.0)
                    },
                    navigation: None,
                },
                &mut rng,
                Some(&mut order),
            );
            mana -= result.mana_spent;
            let Some(expected) =
                case["frames"].as_array().unwrap().iter().find(|frame| frame["tick"] == tick)
            else {
                continue;
            };
            let cast = &wizard.primary_cast;
            let emissions: Vec<_> = result
                .channel_emissions
                .iter()
                .map(|emission| {
                    json!([
                        emission.damage,
                        rows::point(emission.direction),
                        emission.endpoint.map(rows::point),
                        emission.id,
                        "weld",
                        emission.mana_cost,
                        rows::point(emission.origin),
                        rows::point(emission.query_origin),
                        emission.terrain_contact,
                        emission.underpowered,
                    ])
                })
                .collect();
            let actual = json!({"tick":tick,"mana":mana,"spent":result.mana_spent,"underflow":result.mana_underflow,"heading":wizard.heading_index,
                "cast":[cast.action_tick.unwrap_or(-1.0),rows::point(cast.aim_direction),cast.cast_sequence,cast.channel_active,cast.emission_sequence,cast.ether_blast_charge,cast.ether_blast_charge_cue_sequence,cast.fizzle_sequence,cast.held,cast.last_weld_playback_rate,cast.last_weld_sound_variant,cast.one_shot_attack_pose_held,cast.selected_primary_age_ticks,cast.selected_primary_id,cast.target_id,cast.underpowered,cast.weapon_pulse],
                "spells":{"nextId":state.next_id,"projectiles":state.projectiles.iter().map(rows::projectile).collect::<Vec<_>>(),"transients":state.transients.iter().map(rows::transient).collect::<Vec<_>>()},
                "rng":{"indexA":rng.index_a,"indexB":rng.index_b,"words":rng.words.as_slice()},"order":{"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal},"emissions":emissions});
            let mut expected = expected.clone();
            // Steam stores an unrounded atan2 heading. Its source-sized floats,
            // queue membership and every shared random word remain exact below.
            for (index, actor) in
                actual["spells"]["transients"].as_array().unwrap().iter().enumerate()
            {
                if let Some(angle) = actor["physics"]["rotationDegrees"].as_f64() {
                    let reference =
                        expected["spells"]["transients"][index]["physics"]["rotationDegrees"]
                            .as_f64()
                            .unwrap();
                    assert!(
                        (angle - reference).abs() < 1e-9,
                        "{} tick{tick}: compositor heading",
                        case["name"]
                    );
                    expected["spells"]["transients"][index]["physics"]["rotationDegrees"] =
                        json!(angle);
                }
            }
            website_rows::compare(&actual, &expected, &format!("{} tick{tick}", case["name"]));
        }
    }
}
