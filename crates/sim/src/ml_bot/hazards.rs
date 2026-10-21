//! Source `ml-bot-policy/hazards.ts`: projectile and Mage-lightning threat rows.

use crate::Vec2;
use crate::boneyard::collision::closest_point_on_segment;
use crate::enemies::projectiles::{EnemyProjectileKind, MageLightningContact};
use crate::enemies::world::EnemyWorld;
use crate::float::hypot;

use super::numbers::{heading_vector, normalized, signed, unsigned, write};
use super::spec::scales;

pub fn observe_ml_bot_policy_hazards(
    enemies: &EnemyWorld,
    player_id: &str,
    position: Vec2,
    radius: f64,
) -> Vec<f32> {
    let mut rows = Vec::new();
    for projectile in &enemies.projectiles {
        let state = projectile.state();
        let heading = heading_vector(state.heading_deg);
        let velocity = Vec2::new(heading.x * state.speed, heading.y * state.speed);
        let relative = Vec2::new(state.position.x - position.x, state.position.y - position.y);
        rows.push(Hazard {
            already_hit: state.hit_player_ids.iter().any(|id| id == player_id),
            cold: state.cold_slow_ticks > 0,
            poison: state.poison_damage > 0.0,
            damage: state.damage,
            distance: (hypot(relative.x, relative.y) - state.contact_radius).max(0.0),
            homing: state.homing,
            id: state.id,
            kind: if projectile.kind() == EnemyProjectileKind::PoisonPool {
                HazardKind::Area
            } else {
                HazardKind::Projectile
            },
            projectile_kind: Some(projectile.kind()),
            position: state.position,
            radius: state.contact_radius,
            remaining_seconds: state.lifetime_ticks.saturating_sub(state.age_ticks) as f64
                / scales::TICK_RATE,
            targeting_self: state.target_player_id.as_deref() == Some(player_id),
            time_to_contact: impact_time(relative, velocity, state.contact_radius + radius),
            velocity,
        });
    }
    for pulse in &enemies.mage_lightning_pulses {
        let closest = closest_point_on_segment(position, pulse.source, pulse.endpoint);
        let owner = enemies.actors.iter().find(|actor| actor.id == pulse.owner_actor_id);
        let targeting_self = matches!(
            &pulse.contact,
            MageLightningContact::TargetAttached { target_player_id, .. } if target_player_id == player_id
        );
        rows.push(Hazard {
            already_hit: targeting_self,
            cold: false,
            poison: false,
            damage: owner.map_or(0.0, |actor| actor.config.secondary_damage),
            distance: hypot(closest.x - position.x, closest.y - position.y),
            homing: false,
            id: pulse.id,
            kind: HazardKind::Beam,
            projectile_kind: None,
            position: closest,
            radius: 0.0,
            remaining_seconds: 1.0 / scales::TICK_RATE,
            targeting_self,
            time_to_contact: 0.0,
            velocity: Vec2::ZERO,
        });
    }
    rows.sort_by(|left, right| {
        left.distance.total_cmp(&right.distance).then(left.id.cmp(&right.id))
    });
    let mut block = vec![0.0; 12 * 24 + 1];
    for (slot, row) in rows.iter().take(12).enumerate() {
        let direction = normalized(row.position.x - position.x, row.position.y - position.y);
        write(
            &mut block,
            slot * 24,
            &[
                1.0,
                f64::from(row.projectile_kind == Some(EnemyProjectileKind::Arrow)),
                f64::from(row.projectile_kind == Some(EnemyProjectileKind::DemonBomb)),
                f64::from(row.projectile_kind == Some(EnemyProjectileKind::Firebolt)),
                f64::from(row.projectile_kind == Some(EnemyProjectileKind::GuidedMissile)),
                f64::from(row.projectile_kind == Some(EnemyProjectileKind::PoisonPool)),
                f64::from(row.kind == HazardKind::Beam),
                direction.x,
                direction.y,
                unsigned(row.distance, scales::RANGE),
                signed(row.velocity.x, scales::VELOCITY),
                signed(row.velocity.y, scales::VELOCITY),
                unsigned(row.radius, scales::RADIUS),
                unsigned(row.time_to_contact, scales::HAZARD_CONTACT_SECONDS),
                unsigned(row.remaining_seconds, scales::HAZARD_LIFETIME_SECONDS),
                f64::from(row.kind == HazardKind::Projectile),
                f64::from(row.kind == HazardKind::Area),
                f64::from(row.kind == HazardKind::Beam),
                f64::from(row.homing),
                f64::from(row.targeting_self),
                unsigned(row.damage, scales::SKILL_DAMAGE),
                f64::from(row.cold),
                f64::from(row.poison),
                f64::from(row.already_hit),
            ],
        );
    }
    block[12 * 24] = (rows.len().min(12) as f64 / 12.0) as f32;
    block
}

#[derive(PartialEq)]
enum HazardKind {
    Area,
    Beam,
    Projectile,
}

struct Hazard {
    already_hit: bool,
    cold: bool,
    poison: bool,
    damage: f64,
    distance: f64,
    homing: bool,
    id: usize,
    kind: HazardKind,
    projectile_kind: Option<EnemyProjectileKind>,
    position: Vec2,
    radius: f64,
    remaining_seconds: f64,
    targeting_self: bool,
    time_to_contact: f64,
    velocity: Vec2,
}

fn impact_time(relative: Vec2, velocity: Vec2, combined_radius: f64) -> f64 {
    let a = velocity.x * velocity.x + velocity.y * velocity.y;
    if a <= 1e-9 {
        return 0.0;
    }
    let b = 2.0 * (relative.x * velocity.x + relative.y * velocity.y);
    let c = relative.x * relative.x + relative.y * relative.y - combined_radius * combined_radius;
    if c <= 0.0 {
        return 0.0;
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return 0.0;
    }
    let root = (-b - discriminant.sqrt()) / (2.0 * a);
    if root >= 0.0 { root } else { 0.0 }
}
