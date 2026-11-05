//! Fixed-body contacts through the public display timeline and authoritative Hub epoch.
//! Geometry and the retained bench route come from Website parity ledger 013 (Report 57).

use solomon_sim::Vec2;
use solomon_sim::hub::npc::{HUB_SKORCHA_PLACEMENTS, HubSkorcha, HubStudentPopulation};
use solomon_sim::hub::regions::{HubRegion, HubTransition, HubTransitionPhase};
use solomon_sim::hub::world::{HubParticipantMotion, step_hub_participants};
use solomon_sim::hub::{PLAYER_CHARACTER_RADIUS, is_traversable_against_segments};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::rng::NativeRng;
use solomon_sim::session::{GameSession, GameSessionInput, GameSessionOptions};
use solomon_web::presentation::NativePresentationTimeline;

#[derive(Clone, Copy)]
struct FixedActor {
    id: &'static str,
    region: HubRegion,
    position: Vec2,
    radius: f64,
}

impl FixedActor {
    const fn new(id: &'static str, region: HubRegion, x: f64, y: f64, radius: f64) -> Self {
        Self { id, region, position: Vec2::new(x, y), radius }
    }
}

// Independent transcription of the recovered membership, rather than reading the
// implementation's fixed-body iterator to decide which contacts need exercising.
const PERMANENT_ACTORS: [FixedActor; 27] = [
    FixedActor::new("perk-witch", HubRegion::Courtyard, 1340.0, 280.0, 15.0),
    FixedActor::new("potion-trader", HubRegion::Courtyard, 1397.0, 664.0, 30.0),
    FixedActor::new("annalist", HubRegion::Courtyard, 895.5, 455.5, 8.0),
    FixedActor::new("items-trader", HubRegion::Courtyard, 1700.5, 449.5, 25.0),
    FixedActor::new("teacher", HubRegion::Courtyard, 576.5, 710.5, 25.0),
    FixedActor::new("memorator", HubRegion::Mortuary, 628.0, 770.0, 25.0),
    FixedActor::new("painting-0", HubRegion::Mortuary, 512.0, 695.0, 40.0),
    FixedActor::new("painting-1", HubRegion::Mortuary, 350.0, 681.0, 40.0),
    FixedActor::new("painting-2", HubRegion::Mortuary, 673.0, 681.0, 40.0),
    FixedActor::new("painting-3", HubRegion::Mortuary, 744.0, 538.0, 40.0),
    FixedActor::new("painting-4", HubRegion::Mortuary, 590.0, 538.0, 40.0),
    FixedActor::new("painting-5", HubRegion::Mortuary, 434.0, 538.0, 40.0),
    FixedActor::new("painting-6", HubRegion::Mortuary, 279.0, 538.0, 40.0),
    FixedActor::new("painting-7", HubRegion::Mortuary, 354.0, 398.0, 40.0),
    FixedActor::new("painting-8", HubRegion::Mortuary, 512.0, 398.0, 40.0),
    FixedActor::new("painting-9", HubRegion::Mortuary, 670.0, 398.0, 40.0),
    FixedActor::new("librarian", HubRegion::Library, 512.0, 595.0, 55.0),
    FixedActor::new("dowser", HubRegion::Library, 900.0, 642.5, 25.0),
    FixedActor::new("library-prop-0", HubRegion::Library, 239.5, 788.0, 40.0),
    FixedActor::new("library-prop-1", HubRegion::Library, 258.5, 678.5, 40.0),
    FixedActor::new("library-prop-2", HubRegion::Library, 762.0, 732.5, 40.0),
    FixedActor::new("library-prop-3", HubRegion::Library, 831.0, 620.5, 40.0),
    FixedActor::new("storeroom-prop-0", HubRegion::Storeroom, 538.0, 324.0, 40.0),
    FixedActor::new("storeroom-prop-1", HubRegion::Storeroom, 537.5, 434.0, 40.0),
    FixedActor::new("storeroom-prop-2", HubRegion::Storeroom, 536.0, 542.5, 40.0),
    FixedActor::new("arch-chancellor", HubRegion::Office, 514.0, 467.0, 55.0),
    FixedActor::new("office-prop-0", HubRegion::Office, 517.5, 681.0, 40.0),
];
const POLISHER: FixedActor =
    FixedActor::new("story-office-polisher", HubRegion::Office, 566.0, 735.0, 15.0);

fn distance(left: Vec2, right: Vec2) -> f64 {
    (left.x - right.x).hypot(left.y - right.y)
}

fn session_at(region: HubRegion, position: Vec2) -> GameSession {
    let mut session = GameSession::new_participants(
        vec![(
            "local".into(),
            NewGameSelection {
                name: "Local".into(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
        )],
        GameSessionOptions::default(),
    )
    .unwrap();
    let participant = session.participants.get_mut("local").unwrap();
    participant.wizard.region = region;
    participant.wizard.position = position;
    participant.wizard.transition = None;
    participant.college_intro = None;
    participant.player.economy.college_intro_pending = false;
    session.npcs.students = HubStudentPopulation::cold(1, 100);
    session.npcs.students.spawning_enabled = false;
    session.npcs.students.spawn_request_pending = false;
    session.npcs.skorcha.skorcha = None;
    session.npcs.skorcha.transition_ticks_remaining = 1_000_000;
    session
}

fn skorcha_at(variant: usize) -> HubSkorcha {
    HubSkorcha {
        dismissal_index: 0,
        gesture: 0,
        gesture_ticks_remaining: 1000,
        hat_active: false,
        hat_phase_degrees: 0.0,
        hat_rate_degrees_per_tick: 0.0,
        position: HUB_SKORCHA_PLACEMENTS[variant],
        rng: NativeRng::new(123),
        variant,
    }
}

fn step_authority(session: &mut GameSession, movement: Vec2) {
    let participant = session.participants.get_mut("local").unwrap();
    let scale = participant.player.combat.movement_scale()
        * participant.player.derived_stats().movement_factor;
    step_hub_participants(
        &mut [HubParticipantMotion {
            id: "local",
            college_intro_pending: participant.player.economy.college_intro_pending,
            wizard: &mut participant.wizard,
            college_intro: &mut participant.college_intro,
            renderer_ready: true,
            movement,
            movement_scale: scale,
            force_movement_facing: false,
        }],
        &mut session.npcs,
        &mut session.collision_rng,
        &mut session.painter_order,
    );
    session.ticks += 1;
}

fn timeline_for(session: &GameSession, movement: Vec2) -> NativePresentationTimeline {
    let mut timeline = NativePresentationTimeline::new(100, 20);
    let now = session.ticks as f64 * 10.0;
    timeline.push(session, now, "local", None).unwrap();
    timeline
        .sample(now, "local", GameSessionInput { movement, ..Default::default() }, false)
        .unwrap()
        .unwrap();
    timeline
}

fn assert_predicted_tick(
    timeline: &mut NativePresentationTimeline,
    session: &mut GameSession,
    movement: Vec2,
    label: &str,
) -> Vec2 {
    let input = GameSessionInput { movement, ..Default::default() };
    let now = session.ticks as f64 * 10.0;
    timeline.sample(now, "local", input, false).unwrap().unwrap();
    step_authority(session, movement);
    let frame = timeline.sample(now + 10.0, "local", input, false).unwrap().unwrap();
    let expected = session.wizard("local").unwrap();
    let actual = frame.session.wizard("local").unwrap();
    assert_eq!(actual.position, expected.position, "{label} tick {}", session.ticks);
    assert_eq!(frame.session.collision_rng, session.collision_rng, "{label} collision RNG");
    let position = actual.position;
    if session.ticks.is_multiple_of(5) {
        timeline.push(session, now + 10.0, "local", None).unwrap();
    }
    position
}

#[test]
fn hub_prediction_respects_annalist_contact_and_reversal() {
    let start = Vec2::new(874.0, 500.0);
    let mut session = session_at(HubRegion::Courtyard, start);
    let mut timeline = timeline_for(&session, Vec2::new(0.0, -1.0));
    let mut contacted = false;
    for (label, ticks, movement) in [
        ("approach", 180, Vec2::new(0.0, -1.0)),
        ("release", 40, Vec2::ZERO),
        ("reverse", 180, Vec2::new(0.0, 1.0)),
        ("release after reversing", 40, Vec2::ZERO),
    ] {
        for _ in 0..ticks {
            let position = assert_predicted_tick(&mut timeline, &mut session, movement, label);
            let distance = distance(position, Vec2::new(895.5, 455.5));
            assert!(distance >= 33.0, "{label}: fixed NPC penetration at {position:?}");
            contacted |= distance <= 33.11;
        }
    }
    assert!(contacted, "the route must actually contact the annalist");
    assert!(session.wizard("local").unwrap().position.y > start.y, "reversal leaves contact");
}

fn assert_fixed_contact(actor: FixedActor, pending: bool, skorcha: Option<HubSkorcha>) {
    let radius = PLAYER_CHARACTER_RADIUS + actor.radius + 4.0;
    let mut exercised = false;
    for direction in 0..24 {
        let angle = f64::from(direction) * std::f64::consts::PI / 12.0;
        let start = Vec2::new(
            actor.position.x + angle.cos() * radius,
            actor.position.y + angle.sin() * radius,
        );
        if !is_traversable_against_segments(start, PLAYER_CHARACTER_RADIUS, actor.region.segments())
            || PERMANENT_ACTORS.iter().any(|other| {
                other.region == actor.region
                    && other.id != actor.id
                    && distance(start, other.position) < PLAYER_CHARACTER_RADIUS + other.radius
            })
        {
            continue;
        }
        let mut session = session_at(actor.region, start);
        session.participants.get_mut("local").unwrap().player.economy.college_intro_pending =
            pending;
        session.npcs.skorcha.skorcha = skorcha.clone();
        let movement = Vec2::new(-angle.cos(), -angle.sin());
        let mut timeline = timeline_for(&session, movement);
        for _ in 0..70 {
            let position = assert_predicted_tick(&mut timeline, &mut session, movement, actor.id);
            exercised |=
                distance(position, actor.position) <= PLAYER_CHARACTER_RADIUS + actor.radius + 0.11;
        }
        if exercised {
            return;
        }
    }
    panic!("{} has no exercised valid contact approach", actor.id);
}

#[test]
fn hub_prediction_matches_all_27_permanent_fixed_bodies() {
    for actor in PERMANENT_ACTORS {
        assert_fixed_contact(actor, false, None);
    }
}

#[test]
fn hub_prediction_keeps_office_polisher_conditional_on_admission() {
    assert_fixed_contact(POLISHER, true, None);
    let start = Vec2::new(POLISHER.position.x, POLISHER.position.y + 40.5);
    assert!(is_traversable_against_segments(
        start,
        PLAYER_CHARACTER_RADIUS,
        HubRegion::Office.segments(),
    ));
    let mut session = session_at(HubRegion::Office, start);
    let movement = Vec2::new(0.0, -1.0);
    let mut timeline = timeline_for(&session, movement);
    for _ in 0..15 {
        assert_predicted_tick(&mut timeline, &mut session, movement, "completed Office admission");
    }
    assert!(
        distance(session.wizard("local").unwrap().position, POLISHER.position) < 40.0,
        "completed admission must not leave a phantom polisher"
    );
}

#[test]
fn hub_prediction_matches_all_three_present_skorcha_placements() {
    for variant in 0..3 {
        let skorcha = skorcha_at(variant);
        let actor = FixedActor {
            id: "skorcha",
            region: HubRegion::Courtyard,
            position: skorcha.position,
            radius: 10.0,
        };
        assert_fixed_contact(actor, false, Some(skorcha));
    }
}

#[test]
fn hub_prediction_updates_skorcha_presence_only_from_accepted_snapshots() {
    let skorcha = skorcha_at(0);
    let start = Vec2::new(skorcha.position.x + 35.5, skorcha.position.y);
    let movement = Vec2::new(-1.0, 0.0);
    let input = GameSessionInput { movement, ..Default::default() };
    let mut session = session_at(HubRegion::Courtyard, start);
    session.participants.get_mut("local").unwrap().wizard.velocity = Vec2::new(-90.0, 0.0);
    session.npcs.skorcha.skorcha = Some(skorcha.clone());
    session.npcs.skorcha.transition_ticks_remaining = 1;
    let mut timeline = timeline_for(&session, movement);
    let blocked = timeline.sample(50.0, "local", input, false).unwrap().unwrap();
    let blocked = blocked.session.wizard("local").unwrap().position;
    assert!(blocked.x - skorcha.position.x >= 35.0, "unreceived expiry cannot remove a body");
    assert_eq!(session.wizard("local").unwrap().position, start, "sampling is display-only");
    assert!(session.npcs.skorcha.skorcha.is_some(), "sampling cannot advance NPC lifetime");

    session.ticks = 5;
    session.participants.get_mut("local").unwrap().wizard.position = blocked;
    session.npcs.skorcha.skorcha = None;
    timeline.push(&session, 50.0, "local", None).unwrap();
    let absent = timeline.sample(60.0, "local", input, false).unwrap().unwrap();
    assert!(absent.session.wizard("local").unwrap().position.x < blocked.x);

    session.ticks = 10;
    session.participants.get_mut("local").unwrap().wizard.position = start;
    session.npcs.skorcha.skorcha = Some(skorcha.clone());
    timeline.push(&session, 100.0, "local", None).unwrap();
    let returned = timeline.sample(150.0, "local", input, false).unwrap().unwrap();
    assert!(returned.session.wizard("local").unwrap().position.x - skorcha.position.x >= 35.0);
}

#[test]
fn hub_prediction_preserves_scripted_wall_exemption_and_fixed_contacts() {
    for (region, start, target) in [
        (HubRegion::Courtyard, Vec2::new(1200.0, 25.0), Vec2::new(1200.0, -1000.0)),
        (HubRegion::Office, Vec2::new(514.0, 550.0), Vec2::new(514.0, 467.0)),
    ] {
        let mut session = session_at(region, start);
        session.participants.get_mut("local").unwrap().wizard.transition = Some(HubTransition {
            alpha: 0.0,
            source_region: region,
            destination: HubRegion::Mortuary,
            phase: HubTransitionPhase::Outgoing,
            scripted_speed: 1.0,
            scripted_target: target,
        });
        let mut timeline = timeline_for(&session, Vec2::ZERO);
        for _ in 0..15 {
            assert_predicted_tick(&mut timeline, &mut session, Vec2::ZERO, "scripted movement");
        }
        let position = session.wizard("local").unwrap().position;
        if region == HubRegion::Courtyard {
            assert!(position.y < PLAYER_CHARACTER_RADIUS, "scripted input bypasses walls");
        } else {
            assert!((547.0..=547.11).contains(&position.y), "the desk still stops the walk");
        }
    }
}

#[test]
fn hub_prediction_preserves_native_bench_pocket_and_east_escape() {
    let mut session = session_at(HubRegion::Courtyard, Vec2::new(898.0, 500.0));
    let mut timeline = timeline_for(&session, Vec2::new(0.0, -1.0));
    let mut minimum_distance = f64::MAX;
    for (label, ticks, movement) in [
        ("north contact", 180, Vec2::new(0.0, -1.0)),
        ("west entry", 40, Vec2::new(-1.0, 0.0)),
        ("north through rejected separation", 120, Vec2::new(0.0, -1.0)),
        ("release", 40, Vec2::ZERO),
        ("south bench contact", 180, Vec2::new(0.0, 1.0)),
    ] {
        for _ in 0..ticks {
            let position = assert_predicted_tick(&mut timeline, &mut session, movement, label);
            minimum_distance = minimum_distance.min(distance(position, Vec2::new(895.5, 455.5)));
        }
    }
    assert!(minimum_distance < 10.0, "the recovered rejected-correction pocket remains reachable");
    let pocket = session.wizard("local").unwrap().position;
    assert!((830.0..850.0).contains(&pocket.x) && (435.0..445.0).contains(&pocket.y));
    for _ in 0..300 {
        assert_predicted_tick(&mut timeline, &mut session, Vec2::new(1.0, 0.0), "east escape");
    }
    let escaped = session.wizard("local").unwrap().position;
    assert!(escaped.x > 1080.0 && distance(escaped, pocket) > 230.0);
}
