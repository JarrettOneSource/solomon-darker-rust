use super::*;
use solomon_sim::boneyard::loot::{BoneyardLootEvent, BoneyardLootEventType};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::economy::{HubActionFeedback, HubActionFeedbackAction};
use solomon_sim::enemies::model::EnemyEvent;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::NativeSecondaryEventState;
use solomon_sim::session::GameSessionOptions;

#[test]
fn the_fixed_native_context_keeps_notifications_above_gameplay_and_below_hud() {
    let ctx = egui::Context::default();
    let world = LayerId::new(Order::Foreground, egui::Id::unique("notification-test-world"));
    let notification = world_notification_layer_id(false);
    let hud = LayerId::new(Order::Middle, egui::Id::unique("game-hud"));
    let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(40.0, 40.0));
    let mut colors = Vec::new();
    for _ in 0..2 {
        ctx.set_fixed_layer_order(&[world, notification, hud]);
        let output = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 300.0))),
                ..Default::default()
            },
            |ctx| {
                // Deliberately paint in the opposite order. The actual layer
                // queue, rather than call order, must preserve native z order.
                egui::Area::new(egui::Id::unique("game-hud"))
                    .order(Order::Middle)
                    .fixed_pos(egui::Pos2::ZERO)
                    .fade_in(false)
                    .show(ctx, |ui| {
                        ui.set_min_size(vec2(40.0, 40.0));
                        ui.painter().rect_filled(rect, 0.0, egui::Color32::GREEN);
                    });
                ctx.layer_painter(notification).rect_filled(rect, 0.0, egui::Color32::RED);
                ctx.layer_painter(world).rect_filled(rect, 0.0, egui::Color32::BLUE);
            },
        );
        colors = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) => Some(rect.fill),
                _ => None,
            })
            .collect();
        output.drop_without_applying_deltas();
    }
    assert_eq!(colors, [egui::Color32::BLUE, egui::Color32::RED, egui::Color32::GREEN]);
}

fn session() -> GameSession {
    let mut session = GameSession::new(
        "local".into(),
        NewGameSelection {
            name: "Felix".into(),
            element: WizardElement::Air,
            discipline: WizardDiscipline::Earth,
        },
        GameSessionOptions::default(),
    );
    session.install_boneyard(solomon_assets::boneyard::load_boneyard([7; 16]).unwrap()).unwrap();
    session
}
fn loot(id: u64, tick: u64, player: Option<&str>, text: &str) -> BoneyardLootEvent {
    BoneyardLootEvent {
        actor_id: 1,
        event_id: id,
        goodie_id: None,
        phase: None,
        playback_rate: None,
        player_id: player.map(String::from),
        position: Vec2::ZERO,
        sound: None,
        text: Some(text.into()),
        tick,
        event_type: BoneyardLootEventType::LootPickup,
    }
}
fn book(sequence: u64) -> HubActionFeedback {
    HubActionFeedback {
        accepted: true,
        action: HubActionFeedbackAction::ReadSkillBook,
        dowsing_pitch: None,
        dye_session_id: None,
        reason: None,
        sequence,
        skill_book_outcome: Some(NativeSkillBookOutcome::Rank { skill_id: Some(8) }),
        transfer_direction: None,
        transfer_gesture: None,
        unforge_outcome: None,
    }
}
fn cheat(id: usize, tick: u64, player: Option<Option<&str>>) -> EnemyEvent {
    let mut event = EnemyEvent::new(EnemyEventKind::PlayerCheatDeath, 1, tick);
    event.event_id = id;
    event.target_player_id = player.map(|player| player.map(String::from));
    event
}
fn overload(id: u64, tick: u64, player: &str, run_id: &str) -> NativeSecondaryEventState {
    let mut event = NativeSecondaryEventState::new(
        NativeSecondaryEventKind::Overload,
        player.into(),
        format!("boneyard:{run_id}"),
        tick,
    );
    event.event_id = id;
    event
}
fn keys(notifications: &WorldNotifications) -> Vec<String> {
    notifications
        .state
        .as_ref()
        .unwrap()
        .visuals
        .iter()
        .map(|row| row.message.key.to_string())
        .collect()
}

#[test]
fn retained_lanes_are_seeded_and_only_new_local_receipts_are_consumed_once() {
    let mut session = session();
    let run = session.boneyard.as_ref().unwrap().loaded.run_id.clone();
    session.boneyard.as_mut().unwrap().loot_events.push(loot(1, 0, Some("local"), "Health Potion"));
    session.boneyard.as_mut().unwrap().enemy_events.push(cheat(1, 0, Some(Some("local"))));
    session.secondary.events.push(overload(1, 0, "local", &run));
    session.secondary.next_event_id = 2;
    session.participants.get_mut("local").unwrap().player.economy.action_feedback = Some(book(1));
    let mut notifications = WorldNotifications::default();
    notifications.update(&session, "local", true).unwrap();
    assert!(keys(&notifications).is_empty());
    session.ticks = 1;
    session.boneyard.as_mut().unwrap().loot_events.push(loot(2, 1, Some("local"), "DAMAGE x4"));
    session.boneyard.as_mut().unwrap().enemy_events.push(cheat(2, 1, Some(Some("local"))));
    session.secondary.events.push(overload(2, 1, "local", &run));
    session.secondary.next_event_id = 3;
    session.participants.get_mut("local").unwrap().player.economy.action_feedback = Some(book(2));
    notifications.update(&session, "local", true).unwrap();
    assert_eq!(keys(&notifications), ["loot:2", "book:-2", "secondary:2", "combat:2"]);
    let original = notifications.state.as_ref().unwrap().visuals.clone();
    notifications.update(&session, "local", true).unwrap();
    assert_eq!(notifications.state.as_ref().unwrap().visuals, original);
    assert_eq!(original[0].message.tint, 0xd9ba70);
    assert_eq!(original[1].message.tint, 0x8080ff);
    assert_eq!(original[1].message.text, "Magic Missile +1");
    for (id, player) in [(3, Some("remote")), (4, None)] {
        session.boneyard.as_mut().unwrap().loot_events.push(loot(id, 1, player, "SHIELD +1"));
    }
    for (id, player) in [(3, Some(Some("remote"))), (4, Some(None)), (5, None)] {
        session.boneyard.as_mut().unwrap().enemy_events.push(cheat(id, 1, player));
    }
    session.secondary.events.push(overload(3, 1, "remote", &run));
    session.secondary.events.push(overload(4, 1, "local", "other-world"));
    session.secondary.next_event_id = 5;
    notifications.update(&session, "local", true).unwrap();
    assert_eq!(notifications.state.as_ref().unwrap().visuals, original);
    session.ticks = 2;
    notifications.update(&session, "local", true).unwrap();
    let advanced = notifications.state.as_ref().unwrap().visuals.clone();
    assert_ne!(advanced, original);
    notifications.update(&session, "local", true).unwrap();
    assert_eq!(notifications.state.as_ref().unwrap().visuals, advanced);
}

#[test]
fn world_missing_player_terminal_and_rewound_snapshots_never_replay_old_events() {
    let mut session = session();
    let mut notifications = WorldNotifications::default();
    notifications.update(&session, "local", true).unwrap();
    session.ticks = 10;
    session.boneyard.as_mut().unwrap().loot_events.push(loot(
        1,
        10,
        Some("local"),
        "Health Potion",
    ));
    notifications.update(&session, "local", true).unwrap();
    assert_eq!(keys(&notifications), ["loot:1"]);
    session.boneyard.as_mut().unwrap().loaded.run_id = "next-run".into();
    notifications.update(&session, "local", true).unwrap();
    assert!(keys(&notifications).is_empty());
    session.ticks = 11;
    session.boneyard.as_mut().unwrap().loot_events.push(loot(2, 11, Some("local"), "Mana Potion"));
    notifications.update(&session, "local", true).unwrap();
    assert_eq!(keys(&notifications), ["loot:2"]);
    for phase in [GameRunPhase::GameOver, GameRunPhase::Loadout] {
        session.run.phase = phase;
        notifications.update(&session, "local", true).unwrap();
        assert!(keys(&notifications).is_empty());
        assert!(!notifications.state.as_ref().unwrap().visible);
        session.run.phase = GameRunPhase::Active;
        notifications.update(&session, "local", true).unwrap();
        assert!(keys(&notifications).is_empty());
    }
    session.ticks = 1;
    notifications.update(&session, "local", true).unwrap();
    assert!(keys(&notifications).is_empty());
    let local = session.participants.remove("local").unwrap();
    notifications.update(&session, "local", true).unwrap();
    assert!(notifications.state.is_none());
    session.participants.insert("local".into(), local);
    notifications.update(&session, "local", true).unwrap();
    assert!(keys(&notifications).is_empty());
    let selection =
        session.participants.get("local").expect("participant exists").player.selection.clone();
    assert!(session.add_participant("remote".into(), selection));
    session.participants.get_mut("remote").unwrap().player.economy.action_feedback = Some(book(9));
    notifications.update(&session, "remote", true).unwrap();
    assert!(keys(&notifications).is_empty());
    assert_eq!(notifications.state.as_ref().unwrap().player_id, "remote");
    notifications.reset();
    notifications.update(&session, "local", true).unwrap();
    assert!(keys(&notifications).is_empty());
}

#[test]
fn hidden_life_and_tutorial_notifications_still_expire_on_the_accepted_clock() {
    let mut session = session();
    let mut notifications = WorldNotifications::default();
    notifications.update(&session, "local", false).unwrap();
    session.ticks = 1;
    session.boneyard.as_mut().unwrap().loot_events.push(loot(1, 1, Some("local"), "Health Potion"));
    notifications.update(&session, "local", false).unwrap();
    assert_eq!(keys(&notifications), ["loot:1"]);
    assert!(!notifications.state.as_ref().unwrap().visible);
    notifications.update(&session, "local", true).unwrap();
    assert!(notifications.state.as_ref().unwrap().visible);
    for (life, visible) in [
        (PlayerLifeState::Alive, true),
        (PlayerLifeState::LethalPending, true),
        (PlayerLifeState::Dying, false),
        (PlayerLifeState::Spectating, false),
    ] {
        session.participants.get_mut("local").unwrap().player.combat.life_state = life;
        notifications.update(&session, "local", true).unwrap();
        assert_eq!(notifications.state.as_ref().unwrap().visible, visible);
    }
    session.ticks = 400;
    notifications.update(&session, "local", false).unwrap();
    assert!(keys(&notifications).is_empty());
    session.participants.get_mut("local").unwrap().player.combat.life_state =
        PlayerLifeState::Alive;
    notifications.update(&session, "local", true).unwrap();
    assert!(keys(&notifications).is_empty());
    assert!(notifications.state.as_ref().unwrap().visible);
}

#[test]
fn college_book_feedback_uses_the_same_clock_and_rejects_invalid_source_skill_ids() {
    let mut session = session();
    session.boneyard = None;
    session.run.phase = GameRunPhase::Hub;
    let mut notifications = WorldNotifications::default();
    notifications.update(&session, "local", true).unwrap();
    session.ticks = 1;
    session.participants.get_mut("local").unwrap().player.economy.action_feedback = Some(book(1));
    notifications.update(&session, "local", true).unwrap();
    assert_eq!(keys(&notifications), ["book:-1"]);
    session
        .participants
        .get_mut("local")
        .unwrap()
        .player
        .economy
        .action_feedback
        .as_mut()
        .unwrap()
        .skill_book_outcome = Some(NativeSkillBookOutcome::Rank { skill_id: Some(7) });
    notifications.update(&session, "local", true).unwrap();
    assert_eq!(keys(&notifications), ["book:-1"]);
    session
        .participants
        .get_mut("local")
        .unwrap()
        .player
        .economy
        .action_feedback
        .as_mut()
        .unwrap()
        .sequence = 2;
    assert_eq!(
        notifications.update(&session, "local", true).unwrap_err(),
        "Book result must name a native rank-award skill"
    );
}
