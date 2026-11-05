//! Staff admission, strict contact polygons and ordered physical/knockback outputs.

use crate::float;

use super::*;
use crate::actor_physics::ACTOR_SEPARATION_EPSILON;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::profile::WizardElement;
use crate::profile::runtime::PlayerStaffAttackOutcome;
use crate::spells::targeting::{
    PrimaryConeQuery, PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
    primary_circle_targets, primary_cone_targets,
};

#[must_use]
pub fn staff_admission_target<'a>(
    player: &StaffPlayer,
    targets: &'a [StaffTarget],
) -> Option<&'a StaffTarget> {
    targets.iter().find(|target| admits_physical_contact(player, target))
}

#[must_use]
pub fn staff_physical_contact_targets<'a, T: AsRef<StaffTarget>>(
    player: &StaffPlayer,
    targets: &'a [T],
) -> Vec<&'a T> {
    let mut contacts = Vec::new();
    for target in targets {
        if admits_physical_contact(player, target.as_ref()) {
            contacts.push(target);
        }
    }
    contacts
}

fn admits_physical_contact(player: &StaffPlayer, target: &StaffTarget) -> bool {
    let dx = target.position.x - player.position.x;
    let dy = target.position.y - player.position.y;
    let reach = player.collision_radius + target.collision_radius + ACTOR_SEPARATION_EPSILON;
    if dx * dx + dy * dy > reach * reach {
        return false;
    }
    absolute_heading_delta(player.heading_degrees, heading_degrees_from_vector(dx, dy))
        < STAFF_ADMISSION_HEADING_DEGREES
}

#[must_use]
pub fn resolve_staff_physical_contacts(
    action: &PlayerStaffAction,
    targets: &[StaffPhysicalTarget],
    element: WizardElement,
    ether_knockback_chance: f64,
    rng: &mut Rng,
) -> Vec<StaffPhysicalImpact> {
    let mut impacts = Vec::with_capacity(targets.len());
    for physical in targets {
        let target = &physical.target;
        let vertical = rng.draw_scaled_float(1.0, false);
        let pitch = rng.draw_scaled_float(0.1, true);
        let mut contact_knockback_delta = None;
        let mut pike_break_presentation_rng = None;
        if element == WizardElement::Ether {
            let chance = rng.draw_scaled_float(200.0, false);
            if chance > 0.0 && chance <= ether_knockback_chance {
                contact_knockback_delta = Some(direction_with_magnitude(
                    action.base().origin,
                    target.position,
                    STAFF_CONTACT_KNOCKBACK_STEP,
                ));
                if physical.pike {
                    pike_break_presentation_rng = Some(rng.clone());
                    for _ in 0..STAFF_PIKE_BREAK_RNG_WORDS {
                        rng.draw_word();
                    }
                }
            }
        }
        impacts.push(StaffPhysicalImpact {
            contact_knockback_delta,
            pike_break_presentation_rng,
            sound_pitch: fround(1.0 + pitch),
            target_id: target.id.clone(),
            vertical_velocity: fround(-(1.0 + vertical)),
        });
    }
    impacts
}

#[must_use]
pub fn staff_damage_targets<'a, T: AsRef<StaffTarget>>(
    action: &PlayerStaffAction,
    targets: &'a [T],
) -> Vec<&'a T> {
    let base = action.base();
    let mut contacts = Vec::new();
    for target in targets {
        let target_row = target.as_ref();
        let contained = if base.outcome == PlayerStaffAttackOutcome::Whirl {
            circle_contains(base.origin, STAFF_WHIRL_RADIUS, target_row)
        } else {
            point_in_staff_polygon(
                base.origin,
                base.heading_degrees,
                target_row.position,
                base.outcome == PlayerStaffAttackOutcome::CriticalHit,
            )
        };
        if contained {
            contacts.push(target);
        }
    }
    contacts
}

#[must_use]
pub fn staff_knockback_targets<'a, T: AsRef<StaffTarget>>(
    action: &PlayerStaffAction,
    targets: &'a [T],
) -> Vec<&'a T> {
    let base = action.base();
    let Some(arc_degrees) = knockback_arc(base.outcome) else {
        return Vec::new();
    };
    let query_targets = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            let target = target.as_ref();
            let registration_order = target.registration_order.unwrap_or(index);
            PrimarySpellTarget {
                active: true,
                actor_flags: 2,
                attachment: target.position,
                body_radius: target.collision_radius,
                cell_binding_order: registration_order,
                heading_degrees: None,
                grid_position: None,
                id: target.id.clone(),
                kind: PrimarySpellTargetKind::Enemy,
                priority: 0,
                pending_remove: false,
                position: target.position,
                query_lane: PrimarySpellQueryLane::Grid,
                registration_order,
            }
        })
        .collect::<Vec<_>>();
    let radians = base.heading_degrees * std::f64::consts::PI / 180.0;
    let queried = if arc_degrees >= 360.0 {
        primary_circle_targets(base.origin, STAFF_WHIRL_RADIUS, 2, &query_targets)
    } else {
        primary_cone_targets(PrimaryConeQuery {
            actor_mask: 2,
            aim_direction: Vec2::new(float::sin(radians), -float::cos(radians)),
            half_angle_degrees: arc_degrees,
            has_line_of_sight: &|_| true,
            origin: base.origin,
            reach: STAFF_WHIRL_RADIUS,
            targets: &query_targets,
        })
    };
    let by_id = targets
        .iter()
        .map(|target| (target.as_ref().id.as_str(), target))
        .collect::<std::collections::BTreeMap<_, _>>();
    queried.into_iter().map(|target| by_id[target.id.as_str()]).collect()
}

#[must_use]
pub fn staff_contact_damage_per_target(damage: f64, target_count: usize, radial: bool) -> f64 {
    assert!(damage.is_finite() && damage >= 0.0, "staff damage must be finite and non-negative");
    assert!(target_count > 0, "staff target count must be positive");
    if radial { damage } else { damage.min(2.0 * damage / target_count as f64) }
}

#[must_use]
pub fn create_staff_knockback(
    id: u64,
    action: &PlayerStaffAction,
    target_ids: &[String],
    push_strength_factor: f64,
) -> Option<StaffKnockbackActor> {
    assert!(
        push_strength_factor.is_finite() && push_strength_factor >= 0.0,
        "staff push-strength factor must be finite and non-negative"
    );
    let arc_degrees = knockback_arc(action.base().outcome)?;
    let distance = match action.base().outcome {
        PlayerStaffAttackOutcome::Knockback => 150.0,
        PlayerStaffAttackOutcome::CriticalHit | PlayerStaffAttackOutcome::Whirl => 50.0,
        _ => return None,
    };
    Some(StaffKnockbackActor {
        common: new_common(id, action),
        arc_degrees,
        origin: action.base().origin,
        remaining_distance: fround(distance * push_strength_factor),
        target_ids: target_ids.to_vec(),
    })
}

#[must_use]
pub fn step_staff_knockback(
    source: &StaffKnockbackActor,
    world: &mut impl crate::knockback::KnockbackAreaWorld<String>,
    rng: &mut Rng,
) -> StaffKnockbackStep {
    let area = crate::knockback::step_knockback_area(
        crate::knockback::KnockbackArea {
            origin: source.origin,
            remaining_distance: source.remaining_distance,
            target_ids: &source.target_ids,
        },
        rng,
        world,
    );
    let actor = if area.terminal {
        None
    } else {
        let mut actor = source.clone();
        actor.common.age_ticks += 1;
        actor.remaining_distance = area.remaining_distance;
        actor.target_ids = area.target_ids;
        Some(actor)
    };
    StaffKnockbackStep {
        actor,
        camera_displacement: area.camera_displacement,
        dazzled_target_ids: if area.terminal {
            area.heading_perturbations.iter().map(|row| row.target_id.clone()).collect()
        } else {
            Vec::new()
        },
        displacements: area
            .displacements
            .into_iter()
            .map(|row| StaffDisplacement { delta: row.delta, target_id: row.target_id })
            .collect(),
        heading_perturbations: area
            .heading_perturbations
            .into_iter()
            .map(|row| StaffHeadingPerturbation {
                heading_degrees: row.heading_degrees,
                target_id: row.target_id,
            })
            .collect(),
    }
}

#[must_use]
pub fn create_staff_contact_knockback(
    id: u64,
    action: &PlayerStaffAction,
    target_id: &str,
    delta: Vec2,
) -> PlayerStaffContactKnockback {
    PlayerStaffContactKnockback {
        common: new_common(id, action),
        delta,
        remaining_ticks: STAFF_CONTACT_KNOCKBACK_TICKS,
        target_id: target_id.into(),
    }
}

#[must_use]
pub fn step_staff_contact_knockback(
    source: &PlayerStaffContactKnockback,
    target_exists: bool,
) -> StaffContactKnockbackStep {
    if !target_exists {
        return StaffContactKnockbackStep { actor: None, displacement: None };
    }
    let actor = if source.remaining_ticks <= 1 {
        None
    } else {
        let mut actor = source.clone();
        actor.common.age_ticks += 1;
        actor.remaining_ticks -= 1;
        Some(actor)
    };
    StaffContactKnockbackStep { actor, displacement: Some(source.delta) }
}

fn knockback_arc(outcome: PlayerStaffAttackOutcome) -> Option<f64> {
    match outcome {
        PlayerStaffAttackOutcome::Knockback => Some(80.0),
        PlayerStaffAttackOutcome::CriticalHit => Some(60.0),
        PlayerStaffAttackOutcome::Whirl => Some(365.0),
        _ => None,
    }
}

fn point_in_staff_polygon(origin: Vec2, heading: f64, point: Vec2, critical: bool) -> bool {
    let radians = heading * std::f64::consts::PI / 180.0;
    let dx = point.x - origin.x;
    let dy = point.y - origin.y;
    let local_x = dx * float::cos(radians) + dy * float::sin(radians);
    let local_y = -dx * float::sin(radians) + dy * float::cos(radians);
    let (far_y, far_right_x, near_x): (f64, f64, f64) =
        if critical { (-105.0, 60.0, -45.0) } else { (-70.0, 40.0, -30.0) };
    if !(local_y > far_y && local_y < 0.0) {
        return false;
    }
    let progress = (local_y - far_y) / -far_y;
    let half_width = far_right_x + (near_x.abs() - far_right_x) * progress;
    local_x.abs() < half_width
}

fn circle_contains(origin: Vec2, radius: f64, target: &StaffTarget) -> bool {
    let dx = target.position.x - origin.x;
    let dy = target.position.y - origin.y;
    dx * dx + dy * dy < radius * radius + target.collision_radius * target.collision_radius
}

fn direction_with_magnitude(origin: Vec2, target: Vec2, magnitude: f64) -> Vec2 {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let length = hypot(dx, dy);
    if length == 0.0 {
        Vec2::ZERO
    } else {
        Vec2::new(fround(dx / length * magnitude), fround(dy / length * magnitude))
    }
}

fn absolute_heading_delta(first: f64, second: f64) -> f64 {
    (((second - first + 540.0) % 360.0) - 180.0).abs()
}
