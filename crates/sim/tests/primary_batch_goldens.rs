//! Shared source epochs must not age old actors once per participant or reorder RNG.
use serde_json::{Value, json};
use solomon_sim::hub::HubWizard;
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::primary::native_primary_skill_profile;
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};
use solomon_sim::spells::casting::NativePlayerWeaponKind;
use solomon_sim::spells::model::*;
use solomon_sim::spells::targeting::*;
use solomon_sim::spells::*;
use solomon_sim::{NativeRng, Vec2};

#[path = "support/primary_rows.rs"]
mod primary_rows;
#[path = "support/website_rows.rs"]
mod website_rows;
use primary_rows::*;

fn cast_row(wizard: &HubWizard) -> Value {
    let cast = &wizard.primary_cast;
    json!([
        cast.action_tick.unwrap_or(-1.0),
        point(cast.aim_direction),
        cast.cast_sequence,
        cast.channel_active,
        cast.emission_sequence,
        cast.ether_blast_charge,
        cast.ether_blast_charge_cue_sequence,
        cast.fizzle_sequence,
        cast.held,
        cast.one_shot_attack_pose_held,
        cast.selected_primary_age_ticks,
        cast.selected_primary_id.map_or(-1, i64::from),
        cast.target_id,
        cast.underpowered,
        cast.weapon_pulse,
    ])
}

#[test]
fn ordered_players_share_one_primary_epoch_and_retained_owner_queries() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/primary-batch-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let definitions = case["definitions"].as_array().unwrap();
        let ids: Vec<_> = definitions.iter().map(|row| row["id"].as_str().unwrap()).collect();
        let mut wizards = Vec::new();
        let mut profiles = Vec::new();
        let mut mana = Vec::new();
        let mut targets = Vec::new();
        for (index, definition) in definitions.iter().enumerate() {
            let element = match definition["element"].as_str().unwrap() {
                "air" => WizardElement::Air,
                "earth" => WizardElement::Earth,
                "ether" => WizardElement::Ether,
                "fire" => WizardElement::Fire,
                _ => WizardElement::Water,
            };
            wizards.push(HubWizard::at(Vec2::new(100.0 + index as f64 * 70.0, 200.0)));
            let book = NativeSkillBookState::new(element, WizardDiscipline::Arcane);
            profiles
                .push(native_primary_skill_profile(&book, &NativeOffensiveSpellFactors::default()));
            mana.push(definition["mana"].as_f64().unwrap());
            targets.push(vec![PrimarySpellTarget {
                id: format!("enemy:{index}"),
                position: Vec2::new(450.0 + index as f64 * 100.0, -150.0),
                active: true,
                actor_flags: 2,
                attachment: Vec2::new(0.0, -3.0),
                body_radius: 15.0,
                cell_binding_order: index,
                heading_degrees: Some(0.0),
                grid_position: None,
                kind: PrimarySpellTargetKind::Enemy,
                native_priority: 0,
                pending_remove: false,
                query_lane: PrimarySpellQueryLane::Grid,
                registration_order: index,
            }]);
        }
        let mut state = SpellState::default();
        let mut rng = NativeRng::new(7);
        let mut order = WorldPainterOrder { next_actor_ordinal: 49, next_transient_ordinal: 3 };
        for tick in 0..=160 {
            let active = |id: &str| {
                !(name == "shared-flight-and-called-rocks"
                    && (tick >= 155 || (id == "charlie" && tick >= 85)))
            };
            let can_place = |owner: &str, _: Vec2, _: f64| active(owner);
            let can_traverse =
                |actor: PrimarySpellTraversalActor<'_>, _: Vec2, _: Vec2, _: f64, _: u32| {
                    let owner = match actor {
                        PrimarySpellTraversalActor::Projectile(actor) => &actor.common().owner_id,
                        PrimarySpellTraversalActor::EtherealBoulder(actor) => {
                            &actor.common.owner.owner_id
                        }
                    };
                    active(owner)
                };
            let obstruction = |_: Vec2, _: Vec2, _: Option<&str>, _: u32| None;
            let owner_obstruction = |_: &str, _: Vec2, _: Vec2, _: Option<&str>, _: u32| None;
            let range = |start: Vec2, direction: Vec2, _: f64| {
                Vec2::new(start.x + direction.x * 600.0, start.y + direction.y * 600.0)
            };
            let target_query =
                |owner: &str| targets[ids.iter().position(|id| *id == owner).unwrap()].as_slice();
            let epoch = PrimarySpellEpochContext {
                tick,
                enhanced_effects: true,
                targets_for_owner: &target_query,
                can_place: &can_place,
                can_traverse: &can_traverse,
                obstruction: &owner_obstruction,
                navigation: None,
            };
            let mut active_index = 0;
            let mut players = Vec::new();
            for (index, wizard) in wizards.iter_mut().enumerate() {
                let owner_id = ids[index];
                if !active(owner_id) {
                    continue;
                }
                let omitted = name == "college-authority-omission"
                    && owner_id == "charlie"
                    && !(20..65).contains(&tick);
                let authority = (!omitted).then_some(PrimarySpellCastAuthority {
                    weapon_kind: Some(NativePlayerWeaponKind::Staff),
                    alive: true,
                    available_mana: mana[index],
                    cast_progress_factor: 1.0,
                    eligible: tick < 150,
                    plane_active: false,
                    primary_skill: &profiles[index],
                });
                players.push(PrimarySpellPlayer {
                    wizard,
                    authority,
                    input: PrimarySpellInput {
                        aim: Some(Vec2::new(400.0, -100.0)),
                        primary: tick < [110, 120, 70][active_index]
                            && !(name == "college-authority-omission"
                                && owner_id == "charlie"
                                && tick == 20),
                    },
                    context: PrimarySpellWorldContext {
                        enhanced_effects: true,
                        flame_lash_view_bounds: None,
                        player_light_registration: None,
                        tick,
                        view_scale: 1.2,
                        owner_id,
                        world_key: if owner_id == "alpha" && tick >= 50 {
                            "hub:office"
                        } else {
                            "hub:courtyard"
                        },
                        targets: &targets[index],
                        can_place: &can_place,
                        can_traverse: &can_traverse,
                        obstruction: &obstruction,
                        range_endpoint: &range,
                        navigation: None,
                    },
                });
                active_index += 1;
            }
            let result =
                state.step_primary_players(&mut players, &epoch, &mut rng, Some(&mut order));
            drop(players);
            for result in &result.players {
                let index = ids.iter().position(|id| *id == result.owner_id).unwrap();
                mana[index] -= result.mana_spent;
            }
            let Some(expected) =
                case["frames"].as_array().unwrap().iter().find(|row| row["tick"] == tick)
            else {
                continue;
            };
            let players: Vec<_> = result
                .players
                .iter()
                .map(|result| {
                    let index = ids.iter().position(|id| *id == result.owner_id).unwrap();
                    json!({
                        "id": result.owner_id,
                        "mana": mana[index],
                        "spent": result.mana_spent,
                        "underflow": result.mana_underflow,
                        "heading": wizards[index].heading_index,
                        "cast": cast_row(&wizards[index]),
                    })
                })
                .collect();
            let projectiles: Vec<_> = state
                .projectiles
                .iter()
                .map(|actor| {
                    let mut row = projectile_row(actor);
                    row["owner"] = json!(actor.common().owner_id);
                    row["world"] = json!(actor.common().world_key);
                    row
                })
                .collect();
            let transients: Vec<_> = state
                .transients
                .iter()
                .map(|actor| {
                    let mut row = transient_row(actor);
                    row["owner"] = json!(actor.common().owner_id);
                    row["world"] = json!(actor.common().world_key);
                    row
                })
                .collect();
            let emissions: Vec<_> = result
                .tick
                .channel_emissions
                .iter()
                .map(|e| {
                    json!([
                        e.owner_id,
                        e.damage,
                        point(e.direction),
                        e.endpoint.map(point),
                        e.id,
                        match e.kind {
                            PrimarySpellChannelKind::Air => "air",
                            PrimarySpellChannelKind::Water => "water",
                            PrimarySpellChannelKind::Weld => "weld",
                        },
                        e.mana_cost,
                        point(e.origin),
                        point(e.query_origin),
                        e.underpowered,
                    ])
                })
                .collect();
            website_rows::compare(
                &json!({"tick":tick,"players":players,"state":{"nextId":state.next_id,
                "projectiles":projectiles,"transients":transients},"rng":random_state(&rng),
                "order":[order.next_actor_ordinal,order.next_transient_ordinal],"emissions":emissions}),
                expected,
                &format!("{name}.{tick}"),
            );
        }
    }
}
