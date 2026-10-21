//! World-owned Puppet/Goodie projections from `boneyard-world-targets.ts`.

use std::collections::{BTreeMap, BTreeSet};

use crate::Vec2;
use crate::enemies::puppet_hits::{EnemyPuppetHitKind, EnemyPuppetTarget, boneyard_puppet_target};
use crate::secondary::NativeSecondaryActorKind;
use crate::spells::boneyard::boneyard_primary_scenery_targets;
use crate::spells::model::PrimarySpellTransient;
use crate::spells::targeting::{PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind};
use crate::spells::weld::NativeWeldWorldActor;

use super::super::world_light::BoneyardLightEnvironment;
use super::BoneyardWorld;

/// Saved membership of the three source scenery tables, independent of immutable geometry.
/// Native worlds use `None` for the complete authored tables; old Website saves can retain
/// a nonrectangular subset after their versioned target-ID migration.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardSceneryTargetRetention {
    pub earthquake_ids: BTreeSet<usize>,
    #[cfg_attr(
        feature = "persistence",
        serde(default, deserialize_with = "deserialize_earthquake_variants")
    )]
    pub earthquake_variants: BTreeMap<usize, u32>,
    pub primary_ids: BTreeSet<String>,
    pub spell_ids: BTreeSet<String>,
}

/// JSON object keys are strings. Serde's tagged message buffer does not retain the
/// JSON deserializer's numeric-key coercion, so parse this saved index explicitly.
#[cfg(feature = "persistence")]
fn deserialize_earthquake_variants<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<usize, u32>, D::Error> {
    use serde::Deserialize;

    BTreeMap::<String, u32>::deserialize(deserializer)?
        .into_iter()
        .map(|(id, variant)| {
            id.parse::<usize>().map(|id| (id, variant)).map_err(serde::de::Error::custom)
        })
        .collect()
}

impl BoneyardWorld {
    #[must_use]
    pub fn retains_earthquake_scenery_target(&self, id: usize) -> bool {
        self.scenery_target_retention
            .as_ref()
            .is_none_or(|retained| retained.earthquake_ids.contains(&id))
    }

    /// A restored table retains its optional Tree variant independently of scene geometry.
    #[must_use]
    pub fn earthquake_scenery_variant(&self, id: usize) -> Option<u32> {
        if let Some(retained) = &self.scenery_target_retention {
            return retained.earthquake_variants.get(&id).copied();
        }
        self.loaded.scene.objects.get(id).and_then(|object| {
            (object.type_id == 2001).then_some(object.secondary_variant.unwrap_or(0))
        })
    }

    #[must_use]
    pub fn retains_primary_scenery_target(&self, id: &str) -> bool {
        self.scenery_target_retention
            .as_ref()
            .is_none_or(|retained| retained.primary_ids.contains(id))
    }

    #[must_use]
    pub fn retains_scenery_spell_target(&self, id: &str) -> bool {
        self.scenery_target_retention
            .as_ref()
            .is_none_or(|retained| retained.spell_ids.contains(id))
    }

    #[must_use]
    pub fn scenery_gravestone_target_ids(&self) -> Vec<String> {
        let mut targets = Vec::new();
        for object in &self.loaded.scene.objects {
            if object.type_id != 2029
                || self
                    .cleanup_bounds()
                    .is_some_and(|bounds| !inside_bounds(object.position, bounds))
            {
                continue;
            }
            let id = format!("scenery:{}", object.eid);
            if self.retains_scenery_spell_target(&id) {
                targets.push(id);
            }
        }
        targets
    }
}

#[must_use]
pub fn boneyard_goodie_spell_targets(world: &BoneyardWorld) -> Vec<PrimarySpellTarget> {
    let mut targets = Vec::with_capacity(world.loot.goodies.len());
    for goodie in &world.loot.goodies {
        let mut target = boneyard_puppet_target(
            format!("goodie:{}", goodie.id),
            goodie.position,
            0x2004,
            goodie.scenery_registration_ordinal,
            Some(EnemyPuppetHitKind::Goodie),
            PrimarySpellQueryLane::Grid,
        )
        .target;
        target.body_radius = 20.0;
        target.native_priority = 1000;
        target.kind = PrimarySpellTargetKind::Scenery;
        targets.push(target);
    }
    targets
}

#[must_use]
pub fn boneyard_mouth_world_targets(
    world: &BoneyardWorld,
    environment: BoneyardLightEnvironment<'_>,
) -> Vec<EnemyPuppetTarget> {
    let mut targets = Vec::new();
    for target in boneyard_primary_scenery_targets(world) {
        targets.push(EnemyPuppetTarget { target, hit_kind: Some(EnemyPuppetHitKind::Scenery) });
    }
    for target in boneyard_goodie_spell_targets(world) {
        targets.push(EnemyPuppetTarget { target, hit_kind: Some(EnemyPuppetHitKind::Goodie) });
    }
    let world_key = format!("boneyard:{}", world.loaded.run_id);
    if let Some(primary) = environment.primary_spells {
        for actor in &primary.transients {
            let PrimarySpellTransient::Weld(NativeWeldWorldActor::Meteor(actor)) = actor else {
                continue;
            };
            if actor.common.owner.world_key != world_key {
                continue;
            }
            let registration = actor
                .common
                .owner
                .light_registration
                .expect("Weld Meteor has its native actor registration");
            let mut target = boneyard_puppet_target(
                format!("primary:{}", actor.common.owner.id),
                actor.position,
                8,
                registration.registration_ordinal,
                Some(EnemyPuppetHitKind::Meteor),
                PrimarySpellQueryLane::Grid,
            );
            // Source Meteor registers its Puppet grid cell at the origin even while its
            // actual body falls elsewhere. Retain both positions for the native query order.
            target.target.grid_position = Some(Vec2::ZERO);
            targets.push(target);
        }
    }
    if let Some(secondary) = environment.secondary_abilities {
        for actor in &secondary.actors {
            if actor.kind != NativeSecondaryActorKind::Leviathan || actor.world_key != world_key {
                continue;
            }
            let registration =
                actor.light_registration.expect("Leviathan has its native actor registration");
            targets.push(boneyard_puppet_target(
                format!("secondary:{}", actor.id),
                actor.position,
                0x200,
                registration.registration_ordinal,
                Some(EnemyPuppetHitKind::Leviathan),
                PrimarySpellQueryLane::Grid,
            ));
        }
    }
    targets
}

pub(super) fn inside_bounds(point: Vec2, bounds: super::BoneyardBounds) -> bool {
    point.x >= bounds.x
        && point.x <= bounds.x + bounds.width
        && point.y >= bounds.y
        && point.y <= bounds.y + bounds.height
}
