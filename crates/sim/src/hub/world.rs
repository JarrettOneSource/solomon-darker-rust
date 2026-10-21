//! `hub-world.ts`: one ordered actor epoch for all Hub participants and students.
//! Room membership controls actor pairs; every room shares the source collision RNG.

use crate::Vec2;
use crate::actor_physics::{ActorPhysicsBody, ActorPhysicsWorld, resolve_actor_motion};
use crate::hub::college_intro::{
    HubParticipantStepOptions, NativeCollegeIntroState,
    hub_participant_onboarding_collision_bypass_active, plan_hub_participant_movement,
    step_hub_participant_movement,
};
use crate::hub::npc::HubNpcState;
use crate::hub::regions::HubRegion;
use crate::hub::{
    HubWizard, is_traversable_against_segments, move_with_segments, plan_hub_wizard_tick,
};
use crate::painter::WorldPainterOrder;

pub struct HubParticipantMotion<'a> {
    pub id: &'a str,
    pub wizard: &'a mut HubWizard,
    pub college_intro: &'a mut Option<NativeCollegeIntroState>,
    pub college_intro_pending: bool,
    pub renderer_ready: bool,
    pub movement: Vec2,
    pub movement_scale: f64,
    pub force_movement_facing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HubParticipantWorldResult {
    pub id: String,
    pub region_changed: bool,
    pub clear_college_intro_pending: bool,
}

/// Input rows are in source participant/registration order. Planning precedes the
/// single actor solve; admission and ambient registration remain the final world phase.
pub fn step_hub_participants(
    participants: &mut [HubParticipantMotion<'_>],
    npcs: &mut HubNpcState,
    collision_rng: &mut u32,
    painter_order: &mut WorldPainterOrder,
) -> Vec<HubParticipantWorldResult> {
    npcs.skorcha.step();
    let mut bodies = Vec::new();
    let mut membership = Vec::new();
    let mut plans = Vec::with_capacity(participants.len());
    let mut college_targets = Vec::with_capacity(participants.len());
    for participant in participants.iter() {
        let planned = plan_hub_participant_movement(
            participant.wizard,
            participant.college_intro.as_ref(),
            participant.renderer_ready,
        );
        let mut plan = planned.plan.unwrap_or_else(|| {
            plan_hub_wizard_tick(
                participant.wizard.velocity,
                participant.movement,
                participant.movement_scale,
            )
        });
        if participant.force_movement_facing {
            plan.face = true;
        }
        plans.push(plan);
        college_targets.push(planned.college_target);
        bodies.push(ActorPhysicsBody {
            collision_recipient: true,
            id: format!("player-{}", participant.id),
            position: participant.wizard.position,
            delta: plan.delta,
            radius: 25.0,
            push_resistance: 10.0,
            push_strength: 12.0,
            driven: true,
            push_enabled: true,
        });
        membership.push(HubBodyMembership {
            region: participant.wizard.region,
            static_collision: participant.wizard.transition.is_none()
                && participant.college_intro.is_none(),
            onboarding: hub_participant_onboarding_collision_bypass_active(
                participant.wizard,
                participant.college_intro.as_ref(),
                participant.college_intro_pending,
            ),
            college_pending: participant.college_intro_pending,
        });
    }
    let student_plans = npcs.students.plan_routes();
    let student_start = bodies.len();
    for plan in &student_plans {
        let student = &plan.state;
        bodies.push(ActorPhysicsBody {
            collision_recipient: true,
            id: format!("student-{}", student.id),
            position: student.position,
            delta: plan.delta,
            radius: student.profile.radius,
            push_resistance: student.profile.push_resistance,
            push_strength: student.profile.push_strength,
            driven: true,
            push_enabled: true,
        });
        membership.push(HubBodyMembership::fixed(HubRegion::Courtyard));
        membership.last_mut().unwrap().static_collision = student.static_collision_enabled;
    }
    for (id, position, radius) in [
        ("perk-witch", Vec2::new(1340.0, 280.0), 15.0),
        ("potion-trader", Vec2::new(1397.0, 664.0), 30.0),
        ("annalist", Vec2::new(895.5, 455.5), 8.0),
        ("items-trader", Vec2::new(1700.5, 449.5), 25.0),
        ("teacher", Vec2::new(576.5, 710.5), 25.0),
    ] {
        append_fixed(&mut bodies, &mut membership, id, HubRegion::Courtyard, position, radius);
    }
    for region in [HubRegion::Mortuary, HubRegion::Library, HubRegion::Storeroom, HubRegion::Office]
    {
        for actor in region.room_actors() {
            append_fixed(
                &mut bodies,
                &mut membership,
                actor.id,
                region,
                actor.position,
                actor.radius,
            );
        }
        for prop in region.room_props() {
            append_fixed(&mut bodies, &mut membership, prop.id, region, prop.position, prop.radius);
        }
    }
    if participants.iter().any(|participant| {
        participant.college_intro_pending && participant.wizard.region == HubRegion::Office
    }) {
        append_fixed(
            &mut bodies,
            &mut membership,
            "story-office-polisher",
            HubRegion::Office,
            Vec2::new(566.0, 735.0),
            15.0,
        );
    }
    if let Some(skorcha) = &npcs.skorcha.skorcha {
        append_fixed(
            &mut bodies,
            &mut membership,
            "skorcha",
            HubRegion::Courtyard,
            skorcha.position,
            10.0,
        );
    }
    let lookup: std::collections::HashMap<_, _> =
        bodies.iter().enumerate().map(|(index, body)| (body.id.clone(), index)).collect();
    let mut geometry =
        HubWorldGeometry { membership: &membership, lookup: &lookup, rng: collision_rng };
    resolve_actor_motion(
        &mut bodies,
        &mut geometry,
        |mover, other| {
            let mover_membership = &membership[lookup[&mover.id]];
            let other_membership = &membership[lookup[&other.id]];
            if mover_membership.region != other_membership.region {
                return false;
            }
            if mover.id == "story-office-polisher" || other.id == "story-office-polisher" {
                let (player, member) = if mover.id.starts_with("player-") {
                    (mover, mover_membership)
                } else {
                    (other, other_membership)
                };
                return player.id.starts_with("player-") && member.college_pending;
            }
            let counterpart = if mover_membership.onboarding {
                Some(other)
            } else if other_membership.onboarding {
                Some(mover)
            } else {
                None
            };
            counterpart.is_none_or(|body| {
                !body.id.starts_with("player-") && !body.id.starts_with("student-")
            })
        },
        None,
    );
    let mut results = Vec::with_capacity(participants.len());
    for (index, participant) in participants.iter_mut().enumerate() {
        let previous_region = participant.wizard.region;
        participant.wizard.commit(&plans[index], bodies[index].position);
        let result = step_hub_participant_movement(
            participant.wizard,
            participant.college_intro,
            college_targets[index],
            HubParticipantStepOptions {
                college_intro_pending: participant.college_intro_pending,
                renderer_ready: participant.renderer_ready,
            },
        );
        results.push(HubParticipantWorldResult {
            id: participant.id.to_owned(),
            region_changed: participant.wizard.region != previous_region,
            clear_college_intro_pending: result.clear_college_intro_pending,
        });
    }
    let mut students = Vec::with_capacity(student_plans.len());
    for (plan, body) in student_plans.into_iter().zip(&bodies[student_start..]) {
        let mut student = plan.state;
        student.commit(body.position);
        students.push(student);
    }
    npcs.students.admit(students, Some(painter_order));
    npcs.hagatha.step();
    npcs.luthacus.step();
    npcs.potion.step();
    results
}

#[derive(Clone, Copy)]
struct HubBodyMembership {
    region: HubRegion,
    static_collision: bool,
    onboarding: bool,
    college_pending: bool,
}
impl HubBodyMembership {
    fn fixed(region: HubRegion) -> Self {
        Self { region, static_collision: true, onboarding: false, college_pending: false }
    }
}

fn append_fixed(
    bodies: &mut Vec<ActorPhysicsBody>,
    membership: &mut Vec<HubBodyMembership>,
    id: &str,
    region: HubRegion,
    position: Vec2,
    radius: f64,
) {
    bodies.push(ActorPhysicsBody {
        collision_recipient: true,
        id: id.into(),
        position,
        delta: Vec2::ZERO,
        radius,
        push_resistance: 90.0,
        push_strength: 0.0,
        driven: false,
        push_enabled: false,
    });
    membership.push(HubBodyMembership::fixed(region));
}

struct HubWorldGeometry<'a> {
    membership: &'a [HubBodyMembership],
    lookup: &'a std::collections::HashMap<String, usize>,
    rng: &'a mut u32,
}
impl ActorPhysicsWorld for HubWorldGeometry<'_> {
    fn can_place(&self, id: &str, position: Vec2, radius: f64) -> bool {
        let body = self.membership[self.lookup[id]];
        !body.static_collision
            || is_traversable_against_segments(position, radius, body.region.segments())
    }
    fn move_body(&mut self, id: &str, position: Vec2, delta: Vec2, radius: f64) -> Vec2 {
        let body = self.membership[self.lookup[id]];
        if !body.static_collision {
            return Vec2::new(position.x + delta.x, position.y + delta.y);
        }
        let moved = move_with_segments(position, delta, radius, *self.rng, body.region.segments());
        *self.rng = moved.rng_state;
        moved.position
    }
}
