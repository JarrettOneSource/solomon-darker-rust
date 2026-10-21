//! Event-owned flashes use current retained actor sockets, like `NativeEnemyViews::consumeEvent`.

use solomon_sim::Vec2;
use solomon_sim::enemies::EnemyToken;
use solomon_sim::enemies::attack_presentation::{
    EnemyAttackEffectKind, EnemyRawFireBurstKind, native_enemy_attack_effect_plan_at_age,
};
use solomon_sim::enemies::model::EnemyEventKind;
use solomon_sim::enemies::world::EnemyWorld;
use solomon_sim::painter::PainterRegistration;

use super::{EnemyDrawGroup, EnemyView, EnemyViewFrame};

pub(super) struct EnemyAttackEffect {
    kind: EnemyAttackEffectKind,
    position: Vec2,
    spawn_tick: u64,
    registration: Option<PainterRegistration>,
    born_this_paint: bool,
}

impl EnemyView {
    pub(super) fn prepare_attack_effects(
        &mut self,
        world: &EnemyWorld,
        frame: &EnemyViewFrame<'_>,
    ) {
        let tick =
            frame.presentation.map_or(frame.tick as f64, |metadata| metadata.presentation_tick);
        for event in std::mem::take(&mut self.pending_events) {
            // BoneyardScene forwards accepted events directly to NativeEnemyViews.consumeEvent.
            // Drain each accepted event against this paint's actor view, even when interpolation
            // still displays an earlier tick. Constructors paint age zero before normal aging.
            let Some(actor) = world.actors.iter().find(|actor| actor.id == event.actor_id) else {
                continue;
            };
            let family = actor.config.enemy_token;
            let kind = if event.kind == EnemyEventKind::EnemyActionSound
                && family == EnemyToken::Imp
                && event.sound.is_some_and(|sound| sound.id().starts_with("imp-vocal-"))
            {
                EnemyAttackEffectKind::ImpLandingFlare
            } else if event.kind == EnemyEventKind::AttackMarker && family == EnemyToken::Imp {
                EnemyAttackEffectKind::FireBurst(EnemyRawFireBurstKind::ImpContact)
            } else if event.kind == EnemyEventKind::AttackMarker && family == EnemyToken::Demon {
                EnemyAttackEffectKind::FireBurst(EnemyRawFireBurstKind::DemonBombMuzzle)
            } else {
                continue;
            };
            let position = if kind == EnemyAttackEffectKind::ImpLandingFlare {
                actor.position
            } else {
                *self
                    .attack_origins
                    .get(&actor.id)
                    .expect("enemy attack marker requires its current stock muzzle origin")
            };
            if kind == EnemyAttackEffectKind::FireBurst(EnemyRawFireBurstKind::ImpContact) {
                assert!(
                    event.painter_registration.is_some(),
                    "Imp contact requires its transient painter registration"
                );
            }
            self.auxiliary.insert(
                event.event_id,
                EnemyAttackEffect {
                    kind,
                    position,
                    spawn_tick: event.tick,
                    registration: event.painter_registration,
                    born_this_paint: true,
                },
            );
        }
        let mut expired = Vec::new();
        let mut sorted = Vec::new();
        for (id, effect) in &mut self.auxiliary {
            // Source constructors draw age zero once, even when an event arrives after its tick.
            let age = if effect.born_this_paint {
                0.0
            } else {
                (tick - effect.spawn_tick as f64).max(0.0)
            };
            effect.born_this_paint = false;
            let Some(plan) = native_enemy_attack_effect_plan_at_age(effect.kind, *id, age) else {
                expired.push(*id);
                continue;
            };
            let position = Vec2::new(effect.position.x, effect.position.y + plan.offset_y);
            let group = EnemyDrawGroup::sprites(position, plan.layers, false);
            match effect.kind {
                EnemyAttackEffectKind::ImpLandingFlare => self.pre_world.push(group),
                EnemyAttackEffectKind::FireBurst(EnemyRawFireBurstKind::DemonBombMuzzle) => {
                    self.post_world.push(group)
                }
                EnemyAttackEffectKind::FireBurst(EnemyRawFireBurstKind::ImpContact) => {
                    sorted.push((
                        effect.registration.expect("Imp contact retains its transient painter"),
                        position.y,
                        group,
                    ))
                }
            }
        }
        for id in expired {
            self.auxiliary.remove(&id);
        }
        for (registration, y, group) in sorted {
            self.push_root(registration, y, 0.0, None, group);
        }
    }
}
