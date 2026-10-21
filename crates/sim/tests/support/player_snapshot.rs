//! Canonical player component projection shared by command and session replays.

use serde_json::{Value, json};
use solomon_sim::{NativeRng, player::PlayerState};

pub fn rng_snapshot(rng: &NativeRng) -> Value {
    json!({"indexA": rng.index_a, "indexB": rng.index_b, "words": &rng.words[..]})
}

pub fn snapshot(player: &PlayerState, rng: &NativeRng) -> Value {
    let book = &player.book;
    let economy = &player.economy;
    let progression = &player.progression;
    let derived = player.derived_stats();
    let equipped = [&economy.equipment.hat, &economy.equipment.robe, &economy.equipment.weapon]
        .map(|item| item.as_ref().map(|item| item.id));
    let offer = progression.pending_offer.as_ref().map(|offer| {
        let options: Vec<_> = offer
            .options
            .iter()
            .map(|option| {
                let mut value =
                    json!({"skillId": option.skill_id, "targetRank": option.target_rank});
                if option.insight {
                    value["insight"] = json!(true);
                }
                if let Some(build) = option.weld_build_id {
                    value["weldBuildId"] = json!(build);
                }
                value
            })
            .collect();
        let mut value =
            json!({"level": offer.level, "options": options, "sequence": offer.sequence});
        if let Some(index) = offer.automatic_choice_index {
            value["automaticChoiceIndex"] = json!(index);
        }
        value
    });
    json!({
        "book": {
            "advancedUnlocks": book.advanced_unlocks, "disciplineRoot": book.discipline_root,
            "effectiveRanks": &book.effective_ranks[..], "elementRoot": book.element_root,
            "learnedSkillOrder": book.learned_skill_order, "permanentRanks": &book.permanent_ranks[..],
            "primarySkillId": book.primary_skill_id, "weldBuildId": book.weld_build_id,
            "weldComponentRanks": book.weld_component_ranks,
        },
        "gold": economy.gold,
        "inventory": economy.backpack.iter().map(|item| [item.id, u64::from(item.native_type_id), u64::from(item.quantity)]).collect::<Vec<_>>(),
        "equipped": equipped,
        "economyRevision": economy.revision, "ownedPerks": economy.owned_perk_selectors,
        "economyRng": rng_snapshot(&economy.rng), "gameRng": rng_snapshot(rng),
        "resources": [player.combat.current_health, player.combat.current_mana, player.combat.maximum_health, player.combat.maximum_mana],
        "progression": {"level": progression.level, "experience": progression.experience,
            "offerSeed": progression.offer_seed, "offerCycle": progression.offer_cycle,
            "revision": progression.revision, "pendingOffer": offer},
        "concentrations": [player.skill_runtime.concentration_skill_id_a, player.skill_runtime.concentration_skill_id_b],
        "derived": [derived.cast_progress_factor, derived.movement_factor, derived.maximum_health,
            derived.maximum_mana, derived.mana_recovery_per_second, derived.offensive_damage_factor,
            derived.offensive_mana_cost_factor],
    })
}
