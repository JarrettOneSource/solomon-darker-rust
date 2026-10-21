mod support;

use serde_json::Value;
use solomon_sim::NativeRng;
use solomon_sim::economy::{EquipmentSlot, HubInventoryAction};
use solomon_sim::player::PlayerState;
use solomon_sim::profile::{NewGameSelection, PlayerSkillChoice, WizardDiscipline, WizardElement};

#[path = "support/player_snapshot.rs"]
mod player_snapshot;
use player_snapshot::snapshot;

#[test]
fn player_commands_share_real_equipment_resources_skills_and_random_streams() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-state-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let config = &case["config"];
        let element = match config["element"].as_str().unwrap() {
            "ether" => WizardElement::Ether,
            "fire" => WizardElement::Fire,
            "air" => WizardElement::Air,
            _ => unreachable!(),
        };
        let discipline = match config["discipline"].as_str().unwrap() {
            "arcane" => WizardDiscipline::Arcane,
            "body" => WizardDiscipline::Body,
            "mind" => WizardDiscipline::Mind,
            _ => unreachable!(),
        };
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut player = PlayerState::new(
            NewGameSelection { name: "Oracle".into(), element, discipline },
            &mut rng,
        );
        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let action = &step["action"];
            match action["kind"].as_str().unwrap() {
                "initial" => {}
                "resource-input" => {
                    player.combat.current_health = action["health"].as_f64().unwrap();
                    player.combat.current_mana = action["mana"].as_f64().unwrap();
                    player.economy.gold = action["gold"].as_u64().unwrap();
                }
                "inventory" => {
                    let command = &action["command"];
                    let command = match command["type"].as_str().unwrap() {
                        "consume" => HubInventoryAction::Consume {
                            item_id: command["itemId"].as_u64().unwrap(),
                        },
                        "buy-hagatha" => HubInventoryAction::BuyHagatha {
                            selector: command["selector"].as_i64().unwrap() as i32,
                        },
                        "unequip" => HubInventoryAction::Unequip {
                            slot: EquipmentSlot::Hat,
                            destination_sack_id: None,
                            destination_slot: None,
                        },
                        "equip" => HubInventoryAction::Equip {
                            item_id: command["itemId"].as_u64().unwrap(),
                            slot: EquipmentSlot::Hat,
                        },
                        other => panic!("unknown fixture command {other}"),
                    };
                    assert_eq!(
                        player.apply_inventory(&command, &mut rng).transaction.accepted,
                        step["accepted"].as_bool().unwrap(),
                        "{element:?} command {index}"
                    );
                }
                "experience" => {
                    player.grant_experience(action["amount"].as_f64().unwrap(), &mut rng)
                }
                "choice" => assert!(player.choose_skill(
                    PlayerSkillChoice {
                        choice_index: action["choiceIndex"].as_u64().unwrap() as usize,
                        offer_sequence: action["offerSequence"].as_u64().unwrap(),
                        skill_id: action["skillId"].as_u64().unwrap() as usize,
                    },
                    &mut rng
                )),
                "reroll" | "defer" => {
                    let sequence = action["offerSequence"].as_u64().unwrap();
                    let accepted = if action["kind"] == "reroll" {
                        player.reroll_skill_offer(sequence, &mut rng)
                    } else {
                        player.defer_skill_choice(sequence, &mut rng)
                    };
                    assert_eq!(accepted, step["accepted"].as_bool().unwrap());
                }
                other => panic!("unknown fixture action {other}"),
            }
            support::assert_website_json(
                &snapshot(&player, &rng),
                &step["expected"],
                &format!("{element:?} command {index}"),
                &[],
            );
        }
    }
}
