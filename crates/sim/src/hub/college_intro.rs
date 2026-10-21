//! College admission from `native-college-intro.ts`, `hub-participant-movement.ts`
//! and `college-admission-lifecycle.ts`. The session owns this optional state alongside
//! its existing wizard; no second position, character or participant is retained here.
//! `college_intro_goldens.rs` replays the complete admission and source arm operations.

use std::sync::LazyLock;

use crate::appearance::{
    StarterColorOwner, StarterEquipmentAppearance, roll_native_starter_equipment_appearance,
};
use crate::float::{fround, hypot};
use crate::heading::{heading_degrees_from_vector, heading_index};
use crate::hub::interaction::HubInteractionId;
use crate::hub::regions::{HubRegion, HubTransitionPhase, plan_hub_scripted_movement};
use crate::hub::{HubWizard, HubWizardPlan, PLAYER_CHARACTER_RADIUS};
use crate::spline::NaturalSpline;
use crate::{NativeRng, Vec2};

pub const NATIVE_COLLEGE_COURTYARD_PATH: [Vec2; 10] = [
    Vec2::new(972.0, 1044.0),
    Vec2::new(1074.0, 839.0),
    Vec2::new(1119.0, 611.0),
    Vec2::new(1167.0, 441.0),
    Vec2::new(1164.0, 275.0),
    Vec2::new(1095.0, 187.0),
    Vec2::new(1017.0, 193.0),
    Vec2::new(963.0, 178.0),
    Vec2::new(956.0, 105.0),
    Vec2::new(957.0, 27.0),
];
pub const NATIVE_COLLEGE_OFFICE_PATH: [Vec2; 7] = [
    Vec2::new(400.0, 773.0),
    Vec2::new(380.0, 722.0),
    Vec2::new(263.0, 636.0),
    Vec2::new(289.0, 509.0),
    Vec2::new(396.0, 471.0),
    Vec2::new(420.0, 445.0),
    Vec2::new(420.0, 415.0),
];

static COURTYARD_SPLINE: LazyLock<NaturalSpline> =
    LazyLock::new(|| NaturalSpline::new(&NATIVE_COLLEGE_COURTYARD_PATH));
static OFFICE_SPLINE: LazyLock<NaturalSpline> =
    LazyLock::new(|| NaturalSpline::new(&NATIVE_COLLEGE_OFFICE_PATH));
static TITLE_SPLINE: LazyLock<NaturalSpline> = LazyLock::new(|| {
    NaturalSpline::new(&[
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::ZERO,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
    ])
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeCollegeIntroPhase {
    CourtyardWalk,
    OfficeWalk,
    ArchDialogue,
}

impl NativeCollegeIntroPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CourtyardWalk => "courtyard-walk",
            Self::OfficeWalk => "office-walk",
            Self::ArchDialogue => "arch-dialogue",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeCollegeIntroState {
    pub contact_counter: u32,
    pub cover_alpha: f64,
    pub dialogue_sequence: u64,
    pub office_speed: f64,
    pub path_cursor: f64,
    pub phase: NativeCollegeIntroPhase,
    pub title_cursor: f64,
}

impl Default for NativeCollegeIntroState {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeCollegeIntroState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            contact_counter: 0,
            cover_alpha: 1.0,
            dialogue_sequence: 0,
            office_speed: 1.0,
            path_cursor: 0.0,
            phase: NativeCollegeIntroPhase::CourtyardWalk,
            title_cursor: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCollegePathTarget {
    pub path_cursor: f64,
    pub target: Vec2,
}

#[must_use]
pub fn native_college_path_target(
    phase: NativeCollegeIntroPhase,
    source_cursor: f64,
    position: Vec2,
) -> NativeCollegePathTarget {
    let (spline, minimum, offset) = match phase {
        NativeCollegeIntroPhase::CourtyardWalk => (&*COURTYARD_SPLINE, 1.0, 0.0),
        NativeCollegeIntroPhase::OfficeWalk => (&*OFFICE_SPLINE, 0.0, 102.5),
        NativeCollegeIntroPhase::ArchDialogue => panic!("College dialogue has no walking path"),
    };
    let mut path_cursor = source_cursor.max(minimum).min(spline.extent);
    let sample = |cursor| {
        let point = spline.evaluate(cursor);
        Vec2::new(point.x + offset, point.y + offset)
    };
    let mut target = sample(path_cursor);
    while path_cursor < spline.extent && distance_squared(position, target) < 100.0 {
        path_cursor = (path_cursor + 0.25).min(spline.extent);
        target = sample(path_cursor);
    }
    NativeCollegePathTarget { path_cursor, target }
}

#[must_use]
pub fn native_college_path_heading_index(
    phase: NativeCollegeIntroPhase,
    cursor: f64,
    position: Vec2,
) -> usize {
    heading_index(native_college_path_heading_degrees(phase, cursor, position))
}

#[must_use]
pub fn native_college_path_heading_degrees(
    phase: NativeCollegeIntroPhase,
    cursor: f64,
    position: Vec2,
) -> f64 {
    let target = native_college_path_target(phase, cursor, position).target;
    fround(heading_degrees_from_vector(target.x - position.x, target.y - position.y))
}

pub fn step_native_college_title(state: &mut NativeCollegeIntroState) {
    if state.phase != NativeCollegeIntroPhase::CourtyardWalk {
        return;
    }
    state.cover_alpha = fround(state.cover_alpha - fround(0.0005)).max(0.0);
    state.title_cursor = (state.title_cursor + 0.005200000014156103).min(TITLE_SPLINE.extent);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCollegeTitlePresentation {
    pub alpha: f64,
    pub record: usize,
    pub y: f64,
}

#[must_use]
pub fn native_college_title_presentation(
    title_cursor: f64,
    cover_alpha: f64,
) -> NativeCollegeTitlePresentation {
    let alpha = TITLE_SPLINE.evaluate(title_cursor).x.clamp(0.0, 1.0);
    if title_cursor <= 4.0 {
        NativeCollegeTitlePresentation { alpha, record: 7, y: 250.0 }
    } else {
        NativeCollegeTitlePresentation {
            alpha: ((1.0 - cover_alpha.clamp(0.0, 1.0)) * alpha).clamp(0.0, 1.0),
            record: 9,
            y: 450.0,
        }
    }
}

#[must_use]
pub fn native_college_office_speed(path_cursor: f64, current_speed: f64) -> f64 {
    if path_cursor <= 4.0 {
        return 1.0;
    }
    // The source stops decaying after crossing the threshold; it does not clamp back to it.
    if current_speed <= fround(0.5) {
        return current_speed;
    }
    fround(current_speed * fround(0.99000001))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeCollegeContactStep {
    pub activate: bool,
    pub counter: u32,
}

#[must_use]
pub const fn native_college_contact_step(counter: u32, eligible: bool) -> NativeCollegeContactStep {
    if !eligible {
        return NativeCollegeContactStep { activate: false, counter: 0 };
    }
    let next = counter + 2;
    if next > 10 {
        NativeCollegeContactStep { activate: true, counter: 0 }
    } else {
        NativeCollegeContactStep { activate: false, counter: next }
    }
}

#[must_use]
pub fn hub_college_intro_unstarted(intro: Option<&NativeCollegeIntroState>) -> bool {
    intro.is_some_and(|intro| {
        intro.phase == NativeCollegeIntroPhase::CourtyardWalk && intro.title_cursor == 0.0
    })
}

#[must_use]
pub fn hub_college_admission_pre_loadout(wizard: &HubWizard, pending: bool) -> bool {
    pending
        && !wizard.transition.is_some_and(|transition| {
            transition.phase == HubTransitionPhase::Incoming
                && transition.source_region == HubRegion::Office
                && transition.destination == HubRegion::Courtyard
        })
}

#[must_use]
pub fn hub_college_admission_primary_unset(
    wizard: &HubWizard,
    intro: Option<&NativeCollegeIntroState>,
    pending: bool,
) -> bool {
    hub_college_admission_pre_loadout(wizard, pending)
        && (intro.is_some()
            || wizard.region == HubRegion::Office
            || wizard.transition.is_some_and(|transition| {
                transition.phase == HubTransitionPhase::CollegeLoadout
                    || transition.source_region == HubRegion::Office
            }))
}

#[must_use]
pub fn hub_participant_onboarding_collision_bypass_active(
    wizard: &HubWizard,
    intro: Option<&NativeCollegeIntroState>,
    pending: bool,
) -> bool {
    pending
        && (intro.is_some()
            || wizard.region == HubRegion::Office
            || wizard
                .transition
                .is_some_and(|transition| transition.phase == HubTransitionPhase::CollegeLoadout))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubCollegeIntroSpawn {
    pub position: Vec2,
    pub heading_degrees: f64,
    pub heading_index: usize,
}

/// An arm operation never advances gameplay RNG. The session commits the source starter
/// appearance, optional fresh character/participant and idle primary cast atomically.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubCollegeIntroArmPlan {
    pub appearance: StarterEquipmentAppearance,
    pub reset_primary_cast: bool,
    pub spawn: Option<HubCollegeIntroSpawn>,
}

/// The session admits this only in an active Hub run, after confirming the player exists.
#[must_use]
pub fn plan_hub_college_intro_arm(
    wizard: &HubWizard,
    intro: Option<&NativeCollegeIntroState>,
    pending: bool,
    offer_seed: u32,
    selected_primary_id: i32,
) -> Option<HubCollegeIntroArmPlan> {
    if !pending {
        return None;
    }
    let already_passed =
        intro.is_none() && (wizard.region != HubRegion::Courtyard || wizard.transition.is_some());
    let begin = !already_passed && intro.is_none();
    let position = NATIVE_COLLEGE_COURTYARD_PATH[0];
    let spawn = begin.then(|| HubCollegeIntroSpawn {
        position,
        heading_degrees: native_college_path_heading_degrees(
            NativeCollegeIntroPhase::CourtyardWalk,
            0.0,
            position,
        ),
        heading_index: native_college_path_heading_index(
            NativeCollegeIntroPhase::CourtyardWalk,
            0.0,
            position,
        ),
    });
    let mut random = NativeRng::new(i64::from(offer_seed));
    Some(HubCollegeIntroArmPlan {
        appearance: roll_native_starter_equipment_appearance(
            &mut random,
            StarterColorOwner::College,
        ),
        reset_primary_cast: begin
            || intro.is_some()
            || (hub_college_admission_primary_unset(wizard, intro, true)
                && selected_primary_id != -1),
        spawn,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubParticipantMovementPlan {
    pub college_target: Option<NativeCollegePathTarget>,
    /// None delegates to the session's ordinary input plan.
    pub plan: Option<HubWizardPlan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HubParticipantStepOptions {
    pub college_intro_pending: bool,
    pub renderer_ready: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HubParticipantStepResult {
    pub clear_college_intro_pending: bool,
}

/// Plan before actor collision/commit. Renderer readiness holds only an active College intro.
#[must_use]
pub fn plan_hub_participant_movement(
    wizard: &HubWizard,
    intro: Option<&NativeCollegeIntroState>,
    renderer_ready: bool,
) -> HubParticipantMovementPlan {
    let college_walk = intro.filter(|intro| {
        intro.phase != NativeCollegeIntroPhase::ArchDialogue
            && wizard
                .transition
                .is_none_or(|transition| transition.phase != HubTransitionPhase::Outgoing)
    });
    let college_target = college_walk
        .map(|intro| native_college_path_target(intro.phase, intro.path_cursor, wizard.position));
    let waiting = (intro.is_some() && !renderer_ready)
        || intro.is_some_and(|intro| intro.phase == NativeCollegeIntroPhase::ArchDialogue)
        || wizard
            .transition
            .is_some_and(|transition| transition.phase == HubTransitionPhase::CollegeLoadout);
    let plan = if waiting {
        Some(plan_hub_scripted_movement(wizard.position, wizard.position, 1.0))
    } else if let Some(target) = college_target {
        let speed = college_walk
            .filter(|intro| intro.phase == NativeCollegeIntroPhase::OfficeWalk)
            .map_or(1.0, |intro| intro.office_speed);
        Some(plan_hub_scripted_movement(wizard.position, target.target, speed))
    } else {
        wizard.transition.map(|transition| {
            plan_hub_scripted_movement(
                wizard.position,
                transition.scripted_target,
                transition.scripted_speed,
            )
        })
    };
    HubParticipantMovementPlan { college_target, plan }
}

/// Step once after the existing wizard commits. The caller clears the economy pending bit
/// only when this result reports that the confirmed courtyard entrance has finished.
pub fn step_hub_participant_movement(
    wizard: &mut HubWizard,
    intro: &mut Option<NativeCollegeIntroState>,
    target: Option<NativeCollegePathTarget>,
    options: HubParticipantStepOptions,
) -> HubParticipantStepResult {
    if intro.is_some() && !options.renderer_ready {
        return HubParticipantStepResult::default();
    }
    let previous_region = wizard.region;
    let previous_transition = wizard.transition;
    let office_walk = intro.is_some_and(|intro| intro.phase == NativeCollegeIntroPhase::OfficeWalk);
    wizard.step_transition_for_college(options.college_intro_pending, office_walk);
    let clear_college_intro_pending = options.college_intro_pending
        && previous_transition.is_some_and(|transition| {
            transition.phase == HubTransitionPhase::Incoming
                && transition.source_region == HubRegion::Office
                && transition.destination == HubRegion::Courtyard
        })
        && wizard.region == HubRegion::Courtyard
        && wizard.transition.is_none();
    if let Some(intro) = intro {
        if let Some(target) = target
            && intro.phase != NativeCollegeIntroPhase::ArchDialogue
        {
            if intro.phase == NativeCollegeIntroPhase::OfficeWalk {
                intro.office_speed =
                    native_college_office_speed(target.path_cursor, intro.office_speed);
            }
            intro.path_cursor = target.path_cursor;
        }
        step_native_college_title(intro);
        if previous_region == HubRegion::Courtyard && wizard.region == HubRegion::Office {
            intro.contact_counter = 0;
            intro.office_speed = 1.0;
            intro.path_cursor = 0.0;
            intro.phase = NativeCollegeIntroPhase::OfficeWalk;
        }
        if intro.phase == NativeCollegeIntroPhase::OfficeWalk {
            let target = target.map_or(wizard.position, |target| target.target);
            let contact = native_college_contact_step(
                intro.contact_counter,
                native_college_arch_contact_eligible(wizard.position, target),
            );
            intro.contact_counter = contact.counter;
            if contact.activate {
                intro.dialogue_sequence += 1;
                intro.phase = NativeCollegeIntroPhase::ArchDialogue;
            }
        }
    }
    HubParticipantStepResult { clear_college_intro_pending }
}

pub fn acknowledge_hub_college_intro_dialogue(intro: &mut Option<NativeCollegeIntroState>) -> bool {
    if intro.is_some_and(|intro| intro.phase == NativeCollegeIntroPhase::ArchDialogue) {
        *intro = None;
        true
    } else {
        false
    }
}

pub fn confirm_hub_college_intro_loadout(wizard: &mut HubWizard) -> bool {
    let Some(transition) = wizard.transition.as_mut() else {
        return false;
    };
    if transition.phase != HubTransitionPhase::CollegeLoadout {
        return false;
    }
    transition.phase = HubTransitionPhase::Incoming;
    true
}

#[must_use]
pub fn native_college_arch_contact_eligible(position: Vec2, target: Vec2) -> bool {
    let arch = HubInteractionId::ArchChancellor.definition(true).geometry;
    let dx = arch.position.x - position.x;
    let dy = arch.position.y - position.y;
    let maximum = PLAYER_CHARACTER_RADIUS + arch.radius + 1.0;
    if dx * dx + dy * dy > maximum * maximum {
        return false;
    }
    let move_x = target.x - position.x;
    let move_y = target.y - position.y;
    let move_length = hypot(move_x, move_y);
    let target_length = hypot(dx, dy);
    if move_length == 0.0 || target_length == 0.0 {
        return false;
    }
    (move_x * dx + move_y * dy) / (move_length * target_length) >= 0.7
}

fn distance_squared(left: Vec2, right: Vec2) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    dx * dx + dy * dy
}
