//! Discorporeal mouth targets and Puppet hit latches from `enemies/puppet-hits.ts`.

use super::boss_spell::BossSpellKind;
use super::model::EnemyTarget;
use super::projectiles::EnemyProjectile;
use super::targeting::target_eligible;
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::combat::PuppetHitState;
use crate::spells::targeting::{PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyPuppetHitKind {
    Scenery,
    Goodie,
    Arrow,
    Firebolt,
    Meteor,
    Leviathan,
}
impl EnemyPuppetHitKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Scenery => "scenery",
            Self::Goodie => "goodie",
            Self::Arrow => "arrow",
            Self::Firebolt => "firebolt",
            Self::Meteor => "meteor",
            Self::Leviathan => "leviathan",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyPuppetTarget {
    pub target: PrimarySpellTarget,
    pub hit_kind: Option<EnemyPuppetHitKind>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyPuppetHit {
    pub feedback: PuppetHitState,
    pub hit_tick: u64,
    pub kind: EnemyPuppetHitKind,
    pub target_id: String,
}

#[must_use]
pub fn boneyard_puppet_target(
    id: String,
    position: Vec2,
    actor_flags: u32,
    registration_order: usize,
    hit_kind: Option<EnemyPuppetHitKind>,
    query_lane: PrimarySpellQueryLane,
) -> EnemyPuppetTarget {
    EnemyPuppetTarget {
        target: PrimarySpellTarget {
            active: true,
            actor_flags,
            attachment: Vec2::ZERO,
            body_radius: 0.0,
            cell_binding_order: registration_order,
            grid_position: None,
            heading_degrees: None,
            id,
            kind: PrimarySpellTargetKind::Projectile,
            native_priority: 0,
            pending_remove: false,
            position,
            query_lane,
            registration_order,
        },
        hit_kind,
    }
}

impl EnemyWorld {
    pub fn bind_world_puppet_targets(&mut self, targets: &[EnemyPuppetTarget]) {
        let mut ordered: Vec<_> = targets
            .iter()
            .filter(|target| {
                matches!(
                    target.hit_kind,
                    Some(EnemyPuppetHitKind::Meteor | EnemyPuppetHitKind::Leviathan)
                )
            })
            .collect();
        ordered.sort_by_key(|target| target.target.registration_order);
        for target in ordered {
            self.bind_native_query_target(
                &target.target.id,
                target.target.grid_position.unwrap_or(target.target.position),
            );
        }
    }

    #[must_use]
    pub fn mouth_targets(
        &self,
        players: &[EnemyTarget],
        world_targets: &[EnemyPuppetTarget],
        caster_id: Option<usize>,
    ) -> Vec<EnemyPuppetTarget> {
        let mut targets = Vec::new();
        for target in self.spell_targets() {
            if caster_id.is_some_and(|id| target.id == format!("enemy:{id}")) {
                continue;
            }
            targets.push(EnemyPuppetTarget { target, hit_kind: None });
        }
        for player in players {
            if !target_eligible(player) {
                continue;
            }
            let order = self
                .target_cell_bindings
                .get(&player.id)
                .expect("mouth player targets are bound before the actor pass")
                .order;
            targets.push(boneyard_puppet_target(
                format!("target:{}", player.id),
                player.position,
                if player.summoned { 0x800 } else { 1 },
                order,
                None,
                PrimarySpellQueryLane::Grid,
            ));
        }
        for source in world_targets {
            let mut target = source.clone();
            if let Some(binding) = self.target_cell_bindings.get(&target.target.id) {
                target.target.cell_binding_order = binding.order;
            }
            targets.push(target);
        }
        for projectile in &self.projectiles {
            let state = projectile.state();
            match projectile {
                EnemyProjectile::Arrow { .. } | EnemyProjectile::Firebolt(_) => {
                    let arrow = matches!(projectile, EnemyProjectile::Arrow { .. });
                    targets.push(boneyard_puppet_target(
                        format!("projectile:{}", state.id),
                        state.position,
                        if arrow { 0x80 } else { 0x100 },
                        self.painter_registration_ordinal(state.painter_registration),
                        Some(if arrow {
                            EnemyPuppetHitKind::Arrow
                        } else {
                            EnemyPuppetHitKind::Firebolt
                        }),
                        PrimarySpellQueryLane::Transient,
                    ));
                }
                EnemyProjectile::GuidedMissile(_) => {
                    let mut target = boneyard_puppet_target(
                        format!("projectile:{}", state.id),
                        state.position,
                        0x100,
                        state.native_registration_order,
                        None,
                        PrimarySpellQueryLane::Grid,
                    );
                    target.target.cell_binding_order = state.native_cell_binding_order;
                    targets.push(target);
                }
                _ => {}
            }
        }
        for silk in &self.silks {
            let mut target = boneyard_puppet_target(
                format!("silk:{}", silk.id),
                silk.state.position,
                0x1000,
                silk.native_registration_order,
                None,
                PrimarySpellQueryLane::Grid,
            );
            target.target.cell_binding_order = silk.native_cell_binding_order;
            targets.push(target);
        }
        for spell in &self.boss_spells {
            let lane = match spell.kind {
                BossSpellKind::EyeLaser { .. } => PrimarySpellQueryLane::Transient,
                BossSpellKind::SkullMissile { .. } => PrimarySpellQueryLane::Grid,
                _ => continue,
            };
            let id = format!("boss-spell:{}", spell.owner.id);
            let mut target = boneyard_puppet_target(
                id.clone(),
                spell.owner.position,
                0x100,
                spell.owner.painter_registration.registration_ordinal,
                None,
                lane,
            );
            if lane == PrimarySpellQueryLane::Grid {
                target.target.cell_binding_order = self
                    .target_cell_bindings
                    .get(&id)
                    .expect("Skull Missile query is bound at birth and movement")
                    .order;
            }
            targets.push(target);
        }
        targets
    }

    pub fn hit_puppet(&mut self, target: &EnemyPuppetTarget, tick: u64, strength: f64) {
        let Some(kind) = target.hit_kind else { return };
        let mut feedback = PuppetHitState::default();
        feedback.receive(tick, strength);
        let hit =
            EnemyPuppetHit { feedback, hit_tick: tick, kind, target_id: target.target.id.clone() };
        if let Some(prior) =
            self.puppet_hits.iter_mut().find(|prior| prior.target_id == hit.target_id)
        {
            *prior = hit;
        } else {
            self.puppet_hits.push(hit);
        }
    }

    pub fn step_puppet_hits(&mut self, context: &EnemyStepContext<'_>) {
        if self.puppet_hits.is_empty() {
            return;
        }
        let targets = self.mouth_targets(context.players, context.puppet_targets, None);
        self.retain_puppet_hits(&targets);
        for hit in &mut self.puppet_hits {
            if !matches!(hit.kind, EnemyPuppetHitKind::Meteor | EnemyPuppetHitKind::Leviathan) {
                hit.feedback.step(context.tick, 1);
            }
        }
        self.puppet_hits.retain(|hit| hit.feedback.timer > 0.0);
    }

    pub fn retain_puppet_hits(&mut self, targets: &[EnemyPuppetTarget]) {
        self.puppet_hits.retain(|hit| {
            hit.feedback.timer > 0.0
                && targets
                    .iter()
                    .find(|target| target.target.id == hit.target_id)
                    .is_some_and(|target| target.hit_kind == Some(hit.kind))
        });
    }
}
