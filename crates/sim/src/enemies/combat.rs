//! Attack markers and contact damage from Website `enemies/combat.ts`.
use super::EnemyToken;
use super::config::EnemyFamilyConfig;
use super::model::*;
use super::targeting::{target_by_id, target_player_within_attack_reach};
use super::world::EnemyWorld;
use crate::Vec2;
use crate::float;
use crate::float::hypot;
use crate::painter::{WorldManagerLane, WorldPainterOrder};

pub fn attack_marker(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    tick: u64,
    target_player_id: Option<String>,
    order: &mut WorldPainterOrder,
) -> usize {
    let mut event = EnemyEvent::new(EnemyEventKind::AttackMarker, actor.id, tick);
    event.target_player_id = Some(target_player_id);
    if actor.config.enemy_token == EnemyToken::Imp {
        event.painter_registration = Some(order.register(WorldManagerLane::Transient));
    }
    world.emit_event(event)
}

pub fn direct_player_damage(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    target_player_id: Option<&str>,
    event_id: usize,
) {
    let (Some(player), Some(damage)) = (target_player_id, actor.config.primary_damage) else {
        return;
    };
    let (poison_damage, poison_duration) = match actor.config.family {
        EnemyFamilyConfig::Zombie { poison_punch_damage, poison_duration, .. } => {
            (poison_punch_damage, poison_duration)
        }
        _ => (0.0, 0.0),
    };
    let wraith = actor.config.enemy_token == EnemyToken::Wraith;
    let crow_blind_chance_percent = match actor.config.family {
        EnemyFamilyConfig::Heartmonger { blind_mode, .. } => {
            Some([0.0, 5.0, 10.0, 30.0, 100.0][blind_mode as usize])
        }
        _ => None,
    };
    world.player_damage.push(EnemyPlayerDamage {
        hit_strength: None,
        webbed_strength: None,
        actor_id: actor.id,
        physical_damage: if wraith { 0.0 } else { damage },
        magic_damage: if wraith { damage } else { 0.0 },
        cold_slow_ticks: 0,
        dazzle_ticks: if wraith { 50 } else { 0 },
        event_id,
        poison_damage,
        poison_duration,
        poison_contact_damage: None,
        suppress_hit_response: false,
        suppress_flash: false,
        player_id: player.to_owned(),
        source: None,
        mana_damage_maximum_fraction: None,
        tragic_circle: false,
        crow_blind_chance_percent,
    });
}

pub fn direct_contact_player_damage(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    target_player_id: Option<&str>,
    players: &[EnemyTarget],
    center_reach: f64,
    event_id: usize,
) {
    if !target_player_within_attack_reach(actor, target_player_id, players, center_reach) {
        return;
    }
    direct_player_damage(world, actor, target_player_id, event_id);
    if actor.config.enemy_token != EnemyToken::Zombie {
        return;
    }
    let Some(target) = target_by_id(target_player_id, players) else {
        return;
    };
    let dx = target.position.x - actor.position.x;
    let dy = target.position.y - actor.position.y;
    let distance = hypot(dx, dy);
    let radians = actor.heading_deg * std::f64::consts::PI / 180.0;
    let unit = if distance == 0.0 {
        Vec2::new(float::sin(radians), -float::cos(radians))
    } else {
        Vec2::new(dx / distance, dy / distance)
    };
    world.player_knockbacks.push(EnemyPlayerKnockback {
        actor_id: actor.id,
        delta: Vec2::new(unit.x * 10.0, unit.y * 10.0),
        event_id,
        player_id: target.id.clone(),
    });
}
