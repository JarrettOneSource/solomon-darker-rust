//! Articulated stock Golem parts, stable local painter order and death debris.

use super::draws::*;
use super::types::*;
use crate::Vec2;
use crate::float;
use crate::secondary::NativeSecondaryActorKind as Kind;
use crate::secondary::SecondaryActorRead;

const IRON_TINT: u32 = 0x595959;
const DRAW_SCALE: f64 = 1.111_f32 as f64;
const HALF_DRAW_SCALE: f64 = 0.5555_f32 as f64;

pub fn native_golem_presentation_plan(
    actor: &impl SecondaryActorRead,
    frame: f64,
) -> NativeSecondaryPresentationPlan {
    native_golem_presentation_plan_at_age(actor, actor.age_ticks as f64, frame)
}

pub(super) fn native_golem_presentation_plan_at_age(
    actor: &impl SecondaryActorRead,
    age_ticks: f64,
    frame: f64,
) -> NativeSecondaryPresentationPlan {
    assert_eq!(actor.kind, Kind::Golem, "native Golem presentation has authoritative Golem state");
    let golem = actor.golem().copied().expect("Golem presentation retains its articulation");
    let heading =
        actor.rotation_radians * 180.0 / std::f64::consts::PI + golem.action_heading_offset_degrees;
    let facing = native_golem_facing(heading);
    let opposite = native_golem_facing(heading + 180.0);
    let elevation = if age_ticks < 100.0 {
        0.0
    } else if age_ticks < 200.0 {
        -20.0
    } else {
        -40.0
    };
    let tint = if golem.iron { IRON_TINT } else { WHITE };
    let left = Vec2::new(
        golem.left_foot.x + golem.left_foot_bob.x - actor.position.x,
        golem.left_foot.y + golem.left_foot_bob.y - actor.position.y,
    );
    let right = Vec2::new(
        golem.right_foot.x + golem.right_foot_bob.x - actor.position.x,
        golem.right_foot.y + golem.right_foot_bob.y - actor.position.y,
    );
    let center = Vec2::new((left.x + right.x) * 0.5, (left.y + right.y) * 0.5);
    let body = Body { actor: actor.view(), center, heading, elevation };
    let mut records: Vec<(f64, usize, Vec<NativeSecondarySpriteDraw>)> = Vec::new();
    let front = body.part(
        113 + facing,
        "golem-chassis-front",
        if golem.left_limb_mode == 3 { [10.0, 0.0, -5.0] } else { [15.0, 0.0, 0.0] },
        0.0,
        1.0,
        tint,
    );
    let mut front_draws = vec![front.clone()];
    if ![81, 83, 84, 94, 96].contains(&(81 + facing)) {
        let mut draw =
            NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::Golem, 81 + facing);
        draw.blend = NativeSecondaryBlend::Add;
        draw.offset = front.offset;
        draw.role = "golem-chassis-front-additive".into();
        draw.scale_x = actor.scale * DRAW_SCALE;
        draw.scale_y = draw.scale_x;
        draw.tint = green_tint(cosmetic(actor, frame, 1));
        front_draws.push(draw);
    }
    add_record(&mut records, 0.0, front_draws);
    add_record(
        &mut records,
        0.0,
        vec![body.part(129 + facing, "golem-chassis-rear", [-5.0, 0.0, 0.0], 0.0, 1.0, tint)],
    );
    for (entry, overlay, role, overlay_role, lateral) in [
        (145, 177, "golem-chassis-left", "iron-golem-left-overlay", -30.0),
        (161, 193, "golem-chassis-right", "iron-golem-right-overlay", 30.0),
    ] {
        let mut draws = vec![body.part(entry + facing, role, [-5.0, lateral, 5.0], 0.0, 1.0, tint)];
        if golem.iron {
            draws.push(body.part(
                overlay + facing,
                overlay_role,
                [-5.0, lateral, 5.0],
                0.0,
                1.0,
                WHITE,
            ));
        }
        add_record(&mut records, 0.0, draws);
    }
    if age_ticks >= 100.0 {
        let core = golem_point(center, heading, 0.0, 0.0, elevation + 10.0);
        let core_tint = green_tint(cosmetic(actor, frame, 2));
        let mut lower = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 15);
        lower.offset = core;
        lower.role = "golem-core-lower".into();
        lower.scale_x = actor.scale * (2.0 + cosmetic(actor, frame, 3) * 0.25);
        lower.scale_y = lower.scale_x;
        lower.tint = core_tint;
        let mut upper = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 15);
        upper.offset = Vec2::new(core.x, core.y + 5.0);
        upper.role = "golem-core-upper".into();
        upper.scale_x = actor.scale * (1.5 + cosmetic(actor, frame, 4) * 0.25);
        upper.scale_y = upper.scale_x;
        upper.tint = core_tint;
        add_record(&mut records, -50.0, vec![lower, upper]);
        let left_rotation =
            if golem.left_limb_mode == 1 { 45.0 } else { golem.left_foot_rotation_degrees };
        let right_rotation =
            if golem.right_limb_mode == 1 { -45.0 } else { golem.right_foot_rotation_degrees };
        add_record(
            &mut records,
            -50.0,
            vec![body.part(
                if golem.left_limb_mode > 1 { 17 } else { 1 } + facing,
                "golem-limb-left",
                [-5.0, -38.0, 5.0],
                left_rotation,
                1.0,
                tint,
            )],
        );
        add_record(
            &mut records,
            -50.0,
            vec![body.part(
                if golem.right_limb_mode > 1 { 49 } else { 33 } + facing,
                "golem-limb-right",
                [-5.0, 38.0, 5.0],
                right_rotation,
                1.0,
                tint,
            )],
        );
        for (record, role, position, rotation, scale, bias) in [
            (65 + facing, "golem-piece-forward-right", [-20.0, 12.0, 5.0], 0.0, 1.0, -50.0),
            (65 + facing, "golem-piece-forward-left", [-20.0, -12.0, 8.0], 10.0, 1.0, -50.0),
            (65 + facing, "golem-piece-center", [-15.0, 0.0, 15.0], 0.0, 0.8, -70.0),
            (65 + opposite, "golem-piece-rear-right", [1.0, 12.0, 15.0], 0.0, 1.0, -50.0),
            (65 + opposite, "golem-piece-rear-left", [1.0, -12.0, 12.0], 0.0, 1.0, -50.0),
        ] {
            add_record(
                &mut records,
                bias,
                vec![body.part(record, role, position, rotation, scale, tint)],
            );
        }
    }
    records.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut plan = NativeSecondaryPresentationPlan::new(actor.position);
    if age_ticks >= 200.0 {
        plan.draws = connector_draws(actor, frame, left, right, heading, facing, tint);
    }
    for (_, _, draws) in records {
        plan.draws.extend(draws);
    }
    if age_ticks < 200.0 {
        plan.quads.push(NativeSecondaryQuadDraw {
            alpha: actor.alpha
                * float::sin((200.0 - age_ticks) / 200.0 * std::f64::consts::PI)
                * 0.5,
            atlas: Some(NativeSecondaryAtlas::BadGuys),
            blend: NativeSecondaryBlend::Normal,
            entry: Some(36),
            role: "golem-assembly-beam".into(),
            tint: 0x80ff80,
            vertices: [-35.0, -200.0, 35.0, -200.0, -40.0, 25.0, 40.0, 25.0],
        });
    }
    plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
    plan.world_y = actor.position.y + center.y;
    plan
}

struct Body<'a> {
    actor: crate::secondary::SecondaryActorRef<'a>,
    center: Vec2,
    heading: f64,
    elevation: f64,
}
impl Body<'_> {
    fn part(
        &self,
        entry: usize,
        role: &str,
        position: [f64; 3],
        rotation: f64,
        scale: f64,
        tint: u32,
    ) -> NativeSecondarySpriteDraw {
        let mut draw =
            NativeSecondarySpriteDraw::new(&self.actor, NativeSecondaryAtlas::Golem, entry);
        draw.offset = golem_point(
            self.center,
            self.heading,
            position[0],
            position[1],
            self.elevation + position[2],
        );
        draw.role = role.into();
        draw.rotation_radians = degrees_to_radians(rotation);
        draw.scale_x = self.actor.scale * DRAW_SCALE * scale;
        draw.scale_y = draw.scale_x;
        draw.tint = tint;
        draw
    }
}
fn add_record(
    records: &mut Vec<(f64, usize, Vec<NativeSecondarySpriteDraw>)>,
    bias: f64,
    draws: Vec<NativeSecondarySpriteDraw>,
) {
    records.push((draws[0].offset.y + bias, records.len(), draws));
}

fn connector_draws(
    actor: &impl SecondaryActorRead,
    frame: f64,
    left: Vec2,
    right: Vec2,
    heading: f64,
    facing: usize,
    tint: u32,
) -> Vec<NativeSecondarySpriteDraw> {
    let golem = actor.golem().copied().unwrap();
    let center = Vec2::new((left.x + right.x) * 0.5, (left.y + right.y) * 0.5);
    let radians = degrees_to_radians(heading);
    let lateral = Vec2::new(-float::cos(radians), -float::sin(radians));
    let left_end =
        Vec2::new(left.x + golem.left_connector_offset.x, left.y + golem.left_connector_offset.y);
    let right_end = Vec2::new(
        right.x + golem.right_connector_offset.x,
        right.y + golem.right_connector_offset.y,
    );
    let left_joint = Vec2::new(
        golem.left_connector_offset.x + (left.x + center.x + lateral.x * -10.0) * 0.5,
        golem.left_connector_offset.y + (left.y + center.y + lateral.y * -10.0) * 0.5 - 15.0,
    );
    let right_joint = Vec2::new(
        golem.right_connector_offset.x + (right.x + center.x + lateral.x * 10.0) * 0.5,
        golem.right_connector_offset.y + (right.y + center.y + lateral.y * 10.0) * 0.5 - 15.0,
    );
    let endpoint = |position, side| {
        let mut draw =
            NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::Golem, 97 + facing);
        draw.offset = position;
        draw.role = format!("golem-connector-endpoint-{side}");
        draw.scale_x = actor.scale * DRAW_SCALE;
        draw.scale_y = draw.scale_x;
        draw.tint = tint;
        draw
    };
    let cap = |position, side| {
        let mut draw =
            NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::Golem, 65 + facing);
        draw.offset = position;
        draw.role = format!("golem-connector-cap-{side}");
        draw.scale_x = actor.scale * HALF_DRAW_SCALE;
        draw.scale_y = draw.scale_x;
        draw.tint = tint;
        draw
    };
    let mut draws = if left.y < right.y {
        vec![endpoint(left_end, "left"), endpoint(right_end, "right")]
    } else {
        vec![endpoint(right_end, "right"), endpoint(left_end, "left")]
    };
    for (joint, end, side, salt) in
        [(left_joint, left_end, "left", 10), (right_joint, right_end, "right", 12)]
    {
        let scale = 0.75 + cosmetic(actor, frame, salt + 1) * 0.5;
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 15);
        draw.offset = Vec2::new((joint.x * 3.0 + end.x) * 0.25, (joint.y * 3.0 + end.y) * 0.25);
        draw.role = format!("golem-connector-glow-{side}");
        draw.scale_x = actor.scale * scale;
        draw.scale_y = draw.scale_x;
        draw.tint = green_tint(cosmetic(actor, frame, salt));
        draws.push(draw);
    }
    if left.y < right.y {
        draws.extend([cap(left_joint, "left"), cap(right_joint, "right")]);
    } else {
        draws.extend([cap(right_joint, "right"), cap(left_joint, "left")]);
    }
    draws
}
fn golem_point(center: Vec2, heading: f64, forward: f64, lateral: f64, vertical: f64) -> Vec2 {
    let radians = degrees_to_radians(heading);
    let x = float::sin(radians);
    let y = -float::cos(radians);
    Vec2::new(center.x + x * forward + y * lateral, center.y + y * forward - x * lateral + vertical)
}
fn cosmetic(actor: &impl SecondaryActorRead, frame: f64, salt: u64) -> f64 {
    hash_unit(actor.id, frame.floor() as u64 * 31 + salt)
}
fn green_tint(unit: f64) -> u32 {
    (((0.5 + unit * 0.3) * 255.0 + 0.5).floor() as u32) << 16 | 0x00ff80
}

#[must_use]
pub fn native_golem_facing(heading: f64) -> usize {
    let normalized = (heading % 360.0 + 360.0) % 360.0;
    (((normalized.round_ties_even() + 9.0) / 22.0).floor() % 16.0 + 16.0) as usize % 16
}

/// Source creates one independent world painter per fragment and one for the star.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeGolemDeathMember {
    Fragment(usize),
    Star,
}

#[must_use]
pub fn native_golem_death_presentation_plan(
    actor: &impl SecondaryActorRead,
) -> NativeSecondaryPresentationPlan {
    native_golem_death_presentation_plan_at_age(actor, actor.age_ticks as f64)
}

pub(super) fn native_golem_death_presentation_plan_at_age(
    actor: &impl SecondaryActorRead,
    age_ticks: f64,
) -> NativeSecondaryPresentationPlan {
    death_member_plan(actor, age_ticks, None)
}

#[must_use]
pub fn native_golem_death_member_plan(
    actor: &impl SecondaryActorRead,
    age_ticks: f64,
    member: NativeGolemDeathMember,
) -> NativeSecondaryPresentationPlan {
    death_member_plan(actor, age_ticks, Some(member))
}

fn death_member_plan(
    actor: &impl SecondaryActorRead,
    age_ticks: f64,
    member: Option<NativeGolemDeathMember>,
) -> NativeSecondaryPresentationPlan {
    assert_eq!(
        actor.kind,
        Kind::GolemDeath,
        "Golem death presentation requires retained fragments"
    );
    let animation = actor.golem_death().expect("Golem death retains mutable fragments");
    let root = match member {
        Some(NativeGolemDeathMember::Fragment(index)) => animation
            .fragments
            .get(index)
            .and_then(Option::as_ref)
            .map_or(actor.position, |fragment| fragment.motion.position),
        _ => actor.position,
    };
    let tint = if actor.variant == 1 { IRON_TINT } else { WHITE };
    let mut plan = NativeSecondaryPresentationPlan::new(root);
    plan.queue_family = NativeSecondaryQueueFamily::OrdinaryDynamic;
    for (index, fragment) in animation.fragments.iter().enumerate() {
        let Some(fragment) = fragment else { continue };
        if member.is_some_and(|member| member != NativeGolemDeathMember::Fragment(index)) {
            continue;
        }
        let alpha = fragment.life.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            continue;
        }
        let motion = fragment.motion;
        let mut rock =
            NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::DeadHawg, 78 + index % 10);
        rock.alpha = alpha;
        rock.offset =
            Vec2::new(motion.position.x - root.x, motion.position.y - root.y + motion.height);
        rock.rotation_radians = degrees_to_radians(motion.rotation_degrees);
        rock.role = format!("golem-death-rock-{index}");
        rock.tint = tint;
        if actor.enhanced {
            let mut shadow = rock.clone();
            shadow.offset.y = motion.position.y - root.y + 2.0;
            shadow.scale_y = actor.scale * 0.75;
            shadow.tint = 0;
            shadow.role = format!("golem-death-shadow-{index}");
            plan.draws.push(shadow);
        }
        plan.draws.push(rock);
    }
    if age_ticks < 15.0 && member.is_none_or(|member| member == NativeGolemDeathMember::Star) {
        let mut star = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 86);
        star.alpha = 0.75 - age_ticks * 0.05;
        star.blend = NativeSecondaryBlend::Add;
        star.offset = Vec2::new(0.0, -15.0);
        star.role = "golem-death-star".into();
        star.rotation_radians = degrees_to_radians(
            animation.star_rotation_degrees + animation.star_step_degrees * age_ticks,
        );
        star.scale_x = 2.0;
        star.scale_y = 2.0;
        star.tint = 0xa6ffa6;
        plan.draws.push(star);
    }
    plan
}
