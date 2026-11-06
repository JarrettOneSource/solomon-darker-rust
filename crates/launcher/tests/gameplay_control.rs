//! Report 103's hold policy through the shared authority control interface. The resume
//! countdown needs two or more connected, materialized humans, and it never replaces
//! renderer, recovery or close readiness.

use std::collections::BTreeSet;

use solomon_launcher::host::control::{ControlContext, GameplayControl};
use solomon_protocol::{PauseSource, ResumeGraceReason};

fn ids(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn context<'a>(
    connected: &'a BTreeSet<String>,
    materialized: &'a BTreeSet<String>,
) -> ControlContext<'a> {
    ControlContext { active_run: true, skill_barrier: false, connected, materialized }
}

#[test]
fn report103_solo_pause_inventory_and_skills_release_without_a_countdown() {
    let solo = ids(&["a"]);
    let context = context(&solo, &solo);
    for source in [
        PauseSource::PauseMenu,
        PauseSource::Inventory,
        PauseSource::SkillBook,
        PauseSource::SkillSelector,
    ] {
        let mut control = GameplayControl::default();
        assert!(control.request_pause("a", "A", source, &context));
        assert!(control.blocks_tick());
        assert!(control.release_pause("a", true, &context, 100.0));
        assert!(control.pause().is_none());
        assert!(control.resume_grace(100.0).is_none(), "{source:?}");
        assert!(!control.blocks_tick(), "{source:?}");
    }
}

#[test]
fn report103_solo_entry_restart_and_rejoin_keep_readiness_then_release_directly() {
    let solo = ids(&["a"]);
    let absent = BTreeSet::new();
    for reason in [
        ResumeGraceReason::GameStarted,
        ResumeGraceReason::GameRestarted,
        ResumeGraceReason::GameRejoined,
    ] {
        let mut control = GameplayControl::default();
        control.arm_renderers("run", &solo);
        control.begin_loading_grace("a", reason, &solo, &[]);
        let sequence = control.resume_grace(0.0).unwrap().sequence;
        control.resume_ready("a", sequence + 1, &context(&solo, &solo), 10.0);
        control.advance(&context(&solo, &solo), 10_000.0);
        assert_eq!(control.resume_grace(10_000.0).unwrap().remaining_ms, None);
        assert!(control.blocks_tick());

        // A valid receipt still cannot materialize a staged rejoin or finish a picker.
        control.resume_ready("a", sequence, &context(&solo, &absent), 10_000.0);
        assert!(control.resume_grace(10_000.0).is_some());
        let mut barrier = context(&solo, &solo);
        barrier.skill_barrier = true;
        control.advance(&barrier, 10_001.0);
        assert!(control.resume_grace(10_001.0).is_some());

        control.advance(&context(&solo, &solo), 10_002.0);
        assert!(control.resume_grace(10_002.0).is_none(), "{reason:?}");
        assert!(control.blocks_tick(), "renderer still has to be ready");
        control.renderer_ready("a", "stale-run");
        control.advance(&context(&solo, &solo), 10_003.0);
        assert!(control.blocks_tick());
        control.renderer_ready("a", "run");
        control.advance(&context(&solo, &solo), 10_004.0);
        assert!(!control.blocks_tick());
        control.resume_ready("a", sequence, &context(&solo, &solo), 10_005.0);
        assert!(control.resume_grace(10_005.0).is_none(), "duplicate receipt");
    }
}

#[test]
fn report103_only_connected_materialized_humans_create_a_countdown() {
    let connected = ids(&["a", "lobby", "staged-rejoin"]);
    let materialized = ids(&["a", "bot", "offline-wizard"]);
    let mut control = GameplayControl::default();
    control.begin_surface_grace(
        "a",
        ResumeGraceReason::InventoryClosed,
        &context(&connected, &materialized),
        0.0,
    );
    assert!(control.resume_grace(0.0).is_none());
    assert!(!control.blocks_tick());
}

fn departing_peer_releases_solo_hold(timed: bool) {
    let pair = ids(&["a", "b"]);
    let solo = ids(&["a"]);
    let mut control = GameplayControl::default();
    control.arm_renderers("run", &pair);
    control.begin_loading_grace("a", ResumeGraceReason::GameRejoined, &pair, &[]);
    let sequence = control.resume_grace(0.0).unwrap().sequence;
    control.renderer_ready("a", "run");
    control.resume_ready("a", sequence, &context(&pair, &pair), 0.0);
    if timed {
        control.renderer_ready("b", "run");
        control.resume_ready("b", sequence, &context(&pair, &pair), 0.0);
        assert_eq!(control.resume_grace(0.0).unwrap().remaining_ms, Some(2_000));
    }
    control.disconnected("b", &context(&solo, &solo), 10.0);
    assert!(control.resume_grace(10.0).is_none(), "timed={timed}");
    assert!(!control.blocks_tick(), "timed={timed}");
}

#[test]
fn report103_departing_peer_releases_pending_solo_hold() {
    departing_peer_releases_solo_hold(false);
}

#[test]
fn report103_departing_peer_cancels_the_active_countdown_for_solo() {
    departing_peer_releases_solo_hold(true);
}

#[test]
fn report103_required_absent_rejoin_member_still_holds_the_remaining_solo_player() {
    let pair = ids(&["a", "b"]);
    let solo = ids(&["a"]);
    let mut control = GameplayControl::default();
    control.begin_loading_grace("b", ResumeGraceReason::PartyRejoinWait, &pair, &["b".into()]);
    let sequence = control.resume_grace(0.0).unwrap().sequence;
    control.disconnected("b", &context(&solo, &solo), 0.0);
    control.resume_ready("a", sequence, &context(&solo, &solo), 0.0);
    control.advance(&context(&solo, &solo), 10_000.0);
    assert_eq!(control.resume_grace(10_000.0).unwrap().remaining_ms, None);
    assert!(control.blocks_tick());

    control.begin_loading_grace("b", ResumeGraceReason::GameRejoined, &pair, &[]);
    let sequence = control.resume_grace(10_000.0).unwrap().sequence;
    assert_eq!(control.resume_grace(10_000.0).unwrap().reason, ResumeGraceReason::PartyRejoinWait);
    control.resume_ready("b", sequence, &context(&pair, &pair), 10_000.0);
    assert_eq!(control.resume_grace(10_000.0).unwrap().remaining_ms, None);
    control.resume_ready("a", sequence, &context(&pair, &pair), 10_001.0);
    assert_eq!(control.resume_grace(10_001.0).unwrap().remaining_ms, Some(2_000));
    assert!(control.blocks_tick());
}

#[test]
fn report103_multiplayer_orienting_surfaces_retain_the_exact_two_second_countdown() {
    let pair = ids(&["a", "b"]);
    for source in [PauseSource::PauseMenu, PauseSource::Inventory, PauseSource::SkillBook] {
        let mut control = GameplayControl::default();
        assert!(control.request_pause("a", "A", source, &context(&pair, &pair)));
        assert!(!control.release_pause("b", true, &context(&pair, &pair), 100.0));
        assert!(control.release_pause("a", true, &context(&pair, &pair), 100.0));
        assert_eq!(control.resume_grace(100.0).unwrap().remaining_ms, Some(2_000));
        control.advance(&context(&pair, &pair), 2_099.0);
        assert_eq!(control.resume_grace(2_099.0).unwrap().remaining_ms, Some(1));
        assert!(control.blocks_tick());
        control.advance(&context(&pair, &pair), 2_100.0);
        assert!(control.resume_grace(2_100.0).is_none());
        assert!(!control.blocks_tick());
    }
}

#[test]
fn report103_solo_picker_close_keeps_its_presentation_ack_and_skill_barrier() {
    let solo = ids(&["a"]);
    let mut barrier = context(&solo, &solo);
    barrier.skill_barrier = true;
    let mut control = GameplayControl::default();
    control.begin_surface_grace("a", ResumeGraceReason::SkillPickerClosed, &barrier, 0.0);
    let sequence = control.resume_grace(0.0).unwrap().sequence;
    control.resume_ready("a", sequence + 1, &barrier, 0.0);
    control.advance(&context(&solo, &solo), 10.0);
    assert!(control.resume_grace(10.0).is_some());
    control.resume_ready("a", sequence, &barrier, 20.0);
    assert!(control.resume_grace(20.0).is_some());
    control.advance(&context(&solo, &solo), 30.0);
    assert!(control.resume_grace(30.0).is_none());
    assert!(!control.blocks_tick());
}
