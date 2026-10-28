use solomon_sim::profile::{NewGameSelection, PlayerSkillChoice, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

fn session() -> GameSession {
    GameSession::new(
        "wizard".into(),
        NewGameSelection {
            name: "Wizard".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions { gameplay_rng_seed: 17, ..Default::default() },
    )
}

fn pending_choice(game: &GameSession) -> PlayerSkillChoice {
    let offer = game
        .participants
        .get("wizard")
        .unwrap()
        .player
        .progression
        .pending_offer
        .as_ref()
        .expect("a granted level has an offer");
    let index = offer.automatic_choice_index.expect("each level keeps the active random Boast");
    PlayerSkillChoice {
        choice_index: index,
        offer_sequence: offer.sequence,
        skill_id: offer.options[index].skill_id,
    }
}

#[test]
fn every_bulk_granted_level_keeps_the_active_random_boast() {
    let mut game = session();
    game.participants.get_mut("wizard").unwrap().player.economy.npc.boast.selected = Some(3.into());
    game.grant_experience("wizard", 801.0);
    let last_level = game.participants.get("wizard").unwrap().player.progression.level;
    assert!(last_level >= 4, "one grant must cross several levels");
    for level in 2..=last_level {
        let choice = pending_choice(&game);
        assert!(game.choose_skill("wizard", choice), "level {level}");
    }
    let player = &game.participants.get("wizard").unwrap().player;
    assert!(player.progression.pending_offer.is_none());
    assert!(player.progression.pending_levels.is_empty());
    assert!(game.level_up_barrier.is_none());
}

#[test]
fn random_choices_follow_offer_construction_without_changing_other_draws() {
    let mut automatic = session();
    let mut manual = automatic.clone();
    automatic.participants.get_mut("wizard").unwrap().player.economy.npc.boast.selected =
        Some(3.into());
    automatic.grant_experience("wizard", 801.0);
    manual.grant_experience("wizard", 801.0);
    loop {
        let Some(expected_offer) =
            manual.participants.get("wizard").unwrap().player.progression.pending_offer.clone()
        else {
            break;
        };
        let index = manual.gameplay_rng.draw_integer(expected_offer.options.len() as i32) as usize;
        assert!(
            manual
                .participants
                .get_mut("wizard")
                .unwrap()
                .player
                .progression
                .set_automatic_choice(index)
        );
        let expected_offer = manual
            .participants
            .get("wizard")
            .unwrap()
            .player
            .progression
            .pending_offer
            .clone()
            .unwrap();
        assert_eq!(
            automatic.participants.get("wizard").unwrap().player.progression.pending_offer,
            Some(expected_offer.clone()),
        );
        assert_eq!(automatic.gameplay_rng, manual.gameplay_rng);
        let choice = PlayerSkillChoice {
            choice_index: index,
            offer_sequence: expected_offer.sequence,
            skill_id: expected_offer.options[index].skill_id,
        };
        assert!(automatic.choose_skill("wizard", choice));
        assert!(manual.choose_skill("wizard", choice));
    }
    assert_eq!(automatic.gameplay_rng, manual.gameplay_rng);
    assert_eq!(
        automatic.participants.get("wizard").unwrap().player.book,
        manual.participants.get("wizard").unwrap().player.book,
    );
}

#[test]
fn stale_choices_do_not_advance_the_offer_or_random_stream() {
    let mut game = session();
    game.participants.get_mut("wizard").unwrap().player.economy.npc.boast.selected = Some(3.into());
    game.grant_experience("wizard", 801.0);
    let mut choice = pending_choice(&game);
    choice.offer_sequence += 1;
    let before = game.clone();
    assert!(!game.choose_skill("wizard", choice));
    assert_eq!(game.gameplay_rng, before.gameplay_rng);
    assert_eq!(
        game.participants.get("wizard").unwrap().player.progression,
        before.participants.get("wizard").unwrap().player.progression,
    );
    assert_eq!(game.level_up_barrier, before.level_up_barrier);
}

#[test]
fn failed_and_nonrandom_boasts_leave_every_offer_interactive() {
    for (id, failed) in [(0, false), (3, true)] {
        let mut game = session();
        let boast = &mut game.participants.get_mut("wizard").unwrap().player.economy.npc.boast;
        boast.selected = Some(id.into());
        boast.failed = failed;
        game.grant_experience("wizard", 801.0);
        loop {
            let Some(offer) =
                game.participants.get("wizard").unwrap().player.progression.pending_offer.clone()
            else {
                break;
            };
            assert_eq!(offer.automatic_choice_index, None);
            assert!(game.choose_skill(
                "wizard",
                PlayerSkillChoice {
                    choice_index: 0,
                    offer_sequence: offer.sequence,
                    skill_id: offer.options[0].skill_id,
                }
            ));
        }
        assert!(game.level_up_barrier.is_none());
    }
}

#[test]
fn authored_random_boasts_keep_their_resolver_across_all_granted_levels() {
    use solomon_sim::economy::{BoastSelection, HubEquipmentState, HubInventoryItem};
    use solomon_sim::hub::interaction::BoastRules;
    use solomon_sim::modding::content::ModBoastSelection;
    use solomon_sim::modding::extensions::{
        GameSimulationDamageFilterInput, GameSimulationExtensions, GameSimulationLootInput,
        GameSimulationManaFilterInput,
    };

    struct AuthoredBoast;
    impl GameSimulationExtensions for AuthoredBoast {
        fn create_loot_items(&self, _: GameSimulationLootInput) -> Vec<HubInventoryItem> {
            panic!("choosing skills does not create loot")
        }
        fn filter_damage(
            &self,
            _: GameSimulationDamageFilterInput<'_>,
            _: &HubEquipmentState,
        ) -> Result<f64, String> {
            panic!("choosing skills does not filter damage")
        }
        fn filter_mana(
            &self,
            _: GameSimulationManaFilterInput<'_>,
            _: &HubEquipmentState,
        ) -> Result<f64, String> {
            panic!("choosing skills does not filter mana")
        }
        fn has_consumable(&self, _: &str) -> bool {
            false
        }
        fn resolve_boast(&self, selection: &BoastSelection) -> Option<BoastRules<'_>> {
            let BoastSelection::Mod(selection) = selection else { return None };
            (selection.mod_id == "random" && selection.content_id == "1").then_some(BoastRules {
                failure_producers: &[],
                label: "Random spells",
                random_skill_choices: true,
                score_multiplier: 2.0,
                success_wave: 10,
            })
        }
    }
    let mut game = session();
    game.participants.get_mut("wizard").unwrap().player.economy.npc.boast.selected =
        Some(BoastSelection::Mod(ModBoastSelection {
            content_id: "1".into(),
            mod_id: "random".into(),
        }));
    game.grant_experience_with_extensions("wizard", 801.0, Some(&AuthoredBoast));
    let last_level = game.participants.get("wizard").unwrap().player.progression.level;
    for _ in 2..=last_level {
        let choice = pending_choice(&game);
        assert!(game.choose_skill_with_extensions("wizard", choice, Some(&AuthoredBoast)));
    }
    assert!(game.level_up_barrier.is_none());
}
