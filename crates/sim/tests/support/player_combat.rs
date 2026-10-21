use serde_json::{Value, json};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::economy::project_inventory_items;
use solomon_sim::harmful::PlayerFlashResponse;
use solomon_sim::player::PlayerState;
use solomon_sim::profile::{
    NewGameSelection, PlayerConcentrationSlot, WizardDiscipline, WizardElement,
    refresh_player_combat_from_skill_stats,
};
use solomon_sim::spells::casting::PlayerPrimaryCastState;
use solomon_sim::{HubWizard, NativeRng, Vec2};

pub fn configured_player(spec: &Value) -> (PlayerState, HubWizard, PlayerPrimaryCastState) {
    let mut player = PlayerState::new(
        NewGameSelection {
            name: "Oracle".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        &mut NativeRng::new(17),
    );
    player.economy.owned_perk_selectors = spec["perks"]
        .as_array()
        .map(|values| values.iter().map(|value| value.as_u64().unwrap() as u32).collect())
        .unwrap_or_default();
    if spec["removePotions"].as_bool().unwrap_or(false) {
        player.economy.backpack.retain(|item| item.native_type_id != 7001);
    }
    player.economy.npc.boast.selected = Some(9.into());
    player.economy.npc.boast.succeeded = false;
    if let Some(ranks) = spec["ranks"].as_array() {
        for entry in ranks {
            let id = entry[0].as_u64().unwrap() as usize;
            player.book.permanent_ranks[id] = entry[1].as_u64().unwrap() as u32;
            if !player.book.learned_skill_order.contains(&id) {
                player.book.learned_skill_order.push(id);
            }
        }
    }
    if let Some(id) = spec["primaryId"].as_u64() {
        player.book.primary_skill_id = id as usize;
    }
    player.skill_runtime.refresh(&mut player.book, &player.economy);
    if let Some(runtime) = spec["runtime"].as_object() {
        for (key, value) in runtime {
            match key.as_str() {
                "concentrationSkillIdA" => {
                    player.skill_runtime.concentration_skill_id_a =
                        value.as_u64().map(|value| value as usize)
                }
                "harden" => {
                    player.skill_runtime.harden.armor = value["armor"].as_f64().unwrap();
                    player.skill_runtime.harden.coating = value["coating"].as_f64().unwrap();
                }
                "hurricaneCharge" => {
                    player.skill_runtime.hurricane_charge = value.as_f64().unwrap()
                }
                "hurricaneRefreshed" => {
                    player.skill_runtime.hurricane_refreshed = value.as_bool().unwrap()
                }
                "meditationIdleElapsedTicks" => {
                    player.skill_runtime.meditation_idle_elapsed_ticks = value.as_u64().unwrap()
                }
                key => panic!("unsupported runtime fixture field {key}"),
            }
        }
    }
    player.progression.hagatha_runtime.purchase(&player.economy.owned_perk_selectors);
    let derived = player.derived_stats();
    refresh_player_combat_from_skill_stats(&mut player.combat, &derived);
    if let Some(combat) = spec["combat"].as_object() {
        for (key, value) in combat {
            match key.as_str() {
                "currentHealth" => player.combat.current_health = value.as_f64().unwrap(),
                "currentMana" => player.combat.current_mana = value.as_f64().unwrap(),
                "poisonTicksRemaining" => {
                    player.combat.poison_ticks_remaining = value.as_u64().unwrap()
                }
                "poisonDamagePerTick" => {
                    player.combat.poison_damage_per_tick = value.as_f64().unwrap()
                }
                "poisonBeforeCold" => player.combat.poison_before_cold = value.as_bool().unwrap(),
                "coldSlowTicksRemaining" => {
                    player.combat.cold_slow_ticks_remaining = value.as_u64().unwrap()
                }
                "lifeState" => {
                    player.combat.life_state = match value.as_str().unwrap() {
                        "alive" => PlayerLifeState::Alive,
                        "lethal-pending" => PlayerLifeState::LethalPending,
                        "dying" => PlayerLifeState::Dying,
                        "spectating" => PlayerLifeState::Spectating,
                        state => panic!("unknown life state {state}"),
                    }
                }
                key => panic!("unsupported combat fixture field {key}"),
            }
        }
    }
    if let Some(clocks) = spec["potionClocks"].as_object() {
        for (key, value) in clocks {
            match key.as_str() {
                "damageX4TicksRemaining" => {
                    player.progression.damage_x4_ticks_remaining = value.as_u64().unwrap()
                }
                "mindChugTicksRemaining" => {
                    player.progression.mind_chug_ticks_remaining = value.as_u64().unwrap()
                }
                "poisonImmunityTicksRemaining" => {
                    player.progression.poison_immunity_ticks_remaining = value.as_u64().unwrap()
                }
                key => panic!("unsupported potion fixture field {key}"),
            }
        }
    }
    let mut primary = PlayerPrimaryCastState::default();
    if let Some(cast) = spec["primaryCast"].as_object() {
        for (key, value) in cast {
            match key.as_str() {
                "held" => primary.held = value.as_bool().unwrap(),
                "channelActive" => primary.channel_active = value.as_bool().unwrap(),
                "actionTick" => primary.action_tick = Some(value.as_f64().unwrap()),
                "underpowered" => primary.underpowered = value.as_bool().unwrap(),
                key => panic!("unsupported cast fixture field {key}"),
            }
        }
    }
    let mut wizard = HubWizard::at(Vec2::new(321.25, 245.5));
    if spec["velocity"].is_object() {
        wizard.velocity = Vec2::new(
            spec["velocity"]["x"].as_f64().unwrap(),
            spec["velocity"]["y"].as_f64().unwrap(),
        );
    }
    (player, wizard, primary)
}

pub fn player_snapshot(player: &PlayerState) -> Value {
    let combat = &player.combat;
    let progression = &player.progression;
    let runtime = &player.skill_runtime;
    let economy = &player.economy;
    let items = project_inventory_items(&economy.backpack)
        .iter()
        .map(|entry| {
            json!([
                entry.item.id,
                entry.item.native_type_id,
                entry.item.native_subtype,
                entry.item.quantity
            ])
        })
        .collect::<Vec<_>>();
    json!({
        "combat": {
            "hitFeedback": {
                "strength": combat.hit_feedback.strength,
                "tick": combat.hit_feedback.tick,
                "timer": combat.hit_feedback.timer,
            },
            "circleSlowTicksRemaining": combat.circle_slow_ticks_remaining,
            "coldSlowTicksRemaining": combat.cold_slow_ticks_remaining,
            "currentHealth": combat.current_health,
            "currentMana": combat.current_mana,
            "dazzleTicksRemaining": combat.dazzle_ticks_remaining,
            "deathAgeTicks": combat.death_age_ticks,
            "deathEpoch": combat.death_epoch,
            "deathTick": combat.death_tick,
            "lifeState": match combat.life_state {
                PlayerLifeState::Alive => "alive",
                PlayerLifeState::LethalPending => "lethal-pending",
                PlayerLifeState::Dying => "dying",
                PlayerLifeState::Spectating => "spectating",
            },
            "lastDamageTick": combat.last_damage_tick,
            "maximumHealth": combat.maximum_health,
            "maximumMana": combat.maximum_mana,
            "poisonDamagePerTick": combat.poison_damage_per_tick,
            "poisonBeforeCold": combat.poison_before_cold,
            "poisonTicksRemaining": combat.poison_ticks_remaining,
        },
        "progression": {
            "revision": progression.revision,
            "damageX4TicksRemaining": progression.damage_x4_ticks_remaining,
            "mindChugTicksRemaining": progression.mind_chug_ticks_remaining,
            "poisonImmunityTicksRemaining": progression.poison_immunity_ticks_remaining,
            "hagathaRuntime": {
                "cheatDeathCharges": progression.hagatha_runtime.cheat_death_charges,
                "reverieActive": progression.hagatha_runtime.reverie_active,
                "serendipityActive": progression.hagatha_runtime.serendipity_active,
            },
        },
        "runtime": {
            "concentrationSkillIdA": runtime.concentration_skill_id_a,
            "concentrationSkillIdB": runtime.concentration_skill_id_b,
            "harden": { "armor": runtime.harden.armor, "coating": runtime.harden.coating },
            "hardenArmorMaximum": runtime.harden_armor_maximum,
            "hardenArmorPerTick": runtime.harden_armor_per_tick,
            "hurricaneCharge": runtime.hurricane_charge,
            "hurricaneEnabled": runtime.hurricane_enabled,
            "hurricaneRefreshed": runtime.hurricane_refreshed,
            "meditationActivityRampTicks": runtime.meditation_activity_ramp_ticks,
            "meditationIdleElapsedTicks": runtime.meditation_idle_elapsed_ticks,
            "mindstarActive": runtime.mindstar_active,
            "nextConcentrationReplacementSlot": match runtime.next_concentration_replacement_slot {
                PlayerConcentrationSlot::A => "a",
                PlayerConcentrationSlot::B => "b",
            },
            "staffMeleeAlternate": runtime.staff_melee_alternate,
        },
        "economy": {
            "revision": economy.revision,
            "boast": {
                "failed": economy.npc.boast.failed,
                "failureSequence": economy.npc.boast.failure_sequence,
                "selected": economy.npc.boast.selected_native(),
                "succeeded": economy.npc.boast.succeeded,
            },
            "items": items,
        },
    })
}

pub fn rng_snapshot(rng: &NativeRng) -> Value {
    json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":&rng.words[..]})
}
pub fn flash_snapshot(response: &PlayerFlashResponse) -> Value {
    json!({"cameraDisplacement":{"x":response.camera_displacement.x,"y":response.camera_displacement.y},"durationTicks":response.duration_ticks,"growScales":response.grow_scales,"pitch":response.pitch})
}
pub fn assert_snapshot(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64(), "{path}"),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path} length");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                assert_snapshot(a, b, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path} field count");
            for (key, b) in b {
                assert_snapshot(&a[key], b, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
