//! The authoritative five-element contact transaction from
//! `core-server/boneyard-spell-combat.ts` and its `spell-combat` modules.
//! Projectile motion has committed before this step. EnemyWorld owns acceptance
//! and semantic events; secondary status ownership receives the returned patches.

use super::air_water::{draw_native_disintegrate_percentile, draw_native_spell_damage};
use super::earth::{NativeEarthBoulderContactInput, consume_native_earth_boulder_contact};
use super::earth_debris::create_native_weld_boulder_contact_debris_program;
use super::effects::*;
use super::ether::{NATIVE_ETHER_BLAST_CONTACT_RADIUS, native_ether_blast_damage};
use super::fire::native_fire_direct_damage;
use super::model::*;
use super::targeting::*;
use super::weld::*;
use crate::enemies::config::EnemyConfig;
use crate::enemies::model::{EnemyDamageRequest, EnemyDamageResult};
use crate::enemies::projectiles::{EnemyProjectileKind, EnemyProjectileRead};
use crate::enemies::world::EnemyWorld;
use crate::float::{fround, hypot};
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::profile::primary::NativePrimarySkillProfile;
use crate::secondary::NativeSecondaryTargetEffectPatch;
use crate::{NativeRng, Vec2};
use std::collections::{BTreeMap, BTreeSet};

mod fire;
mod hurricane;
mod special_death;
mod weld;
mod weld_channels;
mod weld_forces;
mod weld_persistent;

const NATIVE_WELD_FROST_SLOW_FACTOR: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneyardSpellHitKind {
    Air,
    AirHurricane,
    Earth,
    Ether,
    EtherBlast,
    Fire,
    FireEmber,
    FireExplosion,
    FireGoodImp,
    FirePatch,
    Water,
    WaterHail,
    Weld,
}

impl BoneyardSpellHitKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Air => "air",
            Self::AirHurricane => "air-hurricane",
            Self::Earth => "earth",
            Self::Ether => "ether",
            Self::EtherBlast => "ether-blast",
            Self::Fire => "fire",
            Self::FireEmber => "fire-ember",
            Self::FireExplosion => "fire-explosion",
            Self::FireGoodImp => "fire-good-imp",
            Self::FirePatch => "fire-patch",
            Self::Water => "water",
            Self::WaterHail => "water-hail",
            Self::Weld => "weld",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardSpellHit {
    pub actor_id: usize,
    pub amount: f64,
    pub killed: bool,
    pub owner_id: String,
    pub spell_id: u64,
    pub spell_kind: BoneyardSpellHitKind,
    pub tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardSpellBurnContact {
    pub damage: f64,
    pub owner_id: String,
    pub target_id: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardSpellEtherBurnContact {
    pub owner_id: String,
    pub target_id: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardSpellTargetEffectContact {
    pub patch: NativeSecondaryTargetEffectPatch,
    pub target_id: usize,
    pub world_key: String,
}

/// Damage observers are separate from terminal enemy rewards: native score is
/// recorded on a lethal hit, while XP and loot belong to the later death epoch.
#[derive(Clone, Debug, PartialEq)]
pub enum BoneyardSpellDamageNotice {
    LethalReward { actor_id: usize, enemy: Box<EnemyConfig>, owner_id: String },
    HealthDamage { actor_id: usize, amount: f64, maximum_health: f64, owner_id: String },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneyardSpellCombatResult {
    pub burns: Vec<BoneyardSpellBurnContact>,
    pub ether_burns: Vec<BoneyardSpellEtherBurnContact>,
    pub hits: Vec<BoneyardSpellHit>,
    pub target_effects: Vec<BoneyardSpellTargetEffectContact>,
    /// Source observer order, including reward before attribution on a lethal hit.
    pub damage_notices: Vec<BoneyardSpellDamageNotice>,
}

pub type BoneyardSpellDamageMultiplier<'a> = dyn Fn(usize, BoneyardSpellHitKind, &str) -> f64 + 'a;
pub type BoneyardSpellWorldContact<'a> = dyn Fn(Vec2, Vec2, f64) -> Option<f64> + 'a;
pub type ResolveBoneyardSpellEnemyMovement<'a> = dyn FnMut(usize, Vec2, Vec2, f64) -> Vec2 + 'a;

pub struct BoneyardSpellCombatContext<'a> {
    pub enhanced_effects: bool,
    pub channel_emissions: &'a [PrimarySpellChannelEmission],
    pub fire_actor_contacts: &'a [FireActorContact],
    pub frost_missile_world_contacts: &'a [NativeWeldProjectileState],
    pub tick: u64,
    pub world_key: &'a str,
    pub first_world_contact: Option<&'a BoneyardSpellWorldContact<'a>>,
    pub damage_multiplier: &'a BoneyardSpellDamageMultiplier<'a>,
    pub primary_scenery_targets: &'a [PrimarySpellTarget],
    pub resolve_enemy_movement: &'a mut ResolveBoneyardSpellEnemyMovement<'a>,
    pub fireball_hostile_corridor_length: &'a dyn Fn(&str) -> f64,
    pub light_at: Option<&'a dyn Fn(Vec2) -> f64>,
}

struct TargetRow {
    actor_id: usize,
    current_health: f64,
    maximum_health: f64,
    disintegrate_phase: usize,
    target: PrimarySpellTarget,
}

/// One admitted secondary terminal context. Its contacts run synchronously, while
/// the returned age-zero actors wait until the ordinary primary pass completes.
#[derive(Clone, Copy, Debug)]
pub struct BoneyardSpecialDeathInput<'a> {
    pub payload: super::special_death::NativeSpellSpecialDeathPayload,
    pub victim_id: usize,
    pub position: Vec2,
    pub owner_id: &'a str,
    pub next_spell_id: u64,
    pub tick: u64,
    pub world_key: &'a str,
    pub enhanced_effects: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneyardSpecialDeathResult {
    pub spells: SpellState,
    pub combat: BoneyardSpellCombatResult,
}

/// Source `resolveBoneyardSpecialDeathPayload`: execute only the newborn's
/// receiver pass through the canonical contact transaction, without old actors.
pub fn resolve_boneyard_special_death_payload(
    enemies: &mut EnemyWorld,
    input: BoneyardSpecialDeathInput<'_>,
    rng: &mut NativeRng,
    order: &mut WorldPainterOrder,
) -> BoneyardSpecialDeathResult {
    let mut spells = SpellState { next_id: input.next_spell_id, ..SpellState::default() };
    let mut identity_movement = |_, _, requested, _| requested;
    let context = BoneyardSpellCombatContext {
        enhanced_effects: input.enhanced_effects,
        channel_emissions: &[],
        fire_actor_contacts: &[],
        frost_missile_world_contacts: &[],
        tick: input.tick,
        world_key: input.world_key,
        first_world_contact: None,
        damage_multiplier: &|_, _, _| 1.0,
        primary_scenery_targets: &[],
        resolve_enemy_movement: &mut identity_movement,
        fireball_hostile_corridor_length: &|_| 1600.0,
        light_at: None,
    };
    let combat = resolve_with_order(enemies, &mut spells, context, rng, order, Some(input));
    BoneyardSpecialDeathResult { spells, combat }
}

struct ContactWork<'context, 'world> {
    enemies: &'world mut EnemyWorld,
    rng: &'world mut NativeRng,
    order: &'world mut WorldPainterOrder,
    context: BoneyardSpellCombatContext<'context>,
    source: SpellState,
    next_id: u64,
    consumed_projectiles: BTreeSet<u64>,
    consumed_transients: BTreeSet<u64>,
    updated_projectiles: BTreeMap<u64, PrimarySpellProjectile>,
    updated_transients: BTreeMap<u64, PrimarySpellTransient>,
    active_knockback_target_ids: BTreeSet<String>,
    impact_transients: Vec<PrimarySpellTransient>,
    owned_transients: Vec<PrimarySpellTransient>,
    pending_fire_contacts: Vec<FireActorContact>,
    result: BoneyardSpellCombatResult,
}

/// Mutates the authoritative actors/spells/RNG. Damage-born semantic events remain
/// in `EnemyWorld.events`; callers apply returned secondary patches afterwards.
pub fn resolve_boneyard_spell_combat(
    enemies: &mut EnemyWorld,
    spells: &mut SpellState,
    context: BoneyardSpellCombatContext<'_>,
    rng: &mut NativeRng,
    painter_order: Option<&mut WorldPainterOrder>,
) -> BoneyardSpellCombatResult {
    let mut standalone = WorldPainterOrder {
        next_actor_ordinal: spells.next_id as usize,
        next_transient_ordinal: spells.next_id as usize,
    };
    resolve_with_order(
        enemies,
        spells,
        context,
        rng,
        painter_order.unwrap_or(&mut standalone),
        None,
    )
}

fn resolve_with_order<'context, 'world>(
    enemies: &'world mut EnemyWorld,
    spells: &mut SpellState,
    context: BoneyardSpellCombatContext<'context>,
    rng: &'world mut NativeRng,
    order: &'world mut WorldPainterOrder,
    special_death: Option<BoneyardSpecialDeathInput<'_>>,
) -> BoneyardSpellCombatResult {
    let source = std::mem::take(spells);
    let next_id = source.next_id;
    let pending_fire_contacts = context.fire_actor_contacts.to_vec();
    let mut active_knockback_target_ids = BTreeSet::new();
    for effect in &source.transients {
        if effect.common().world_key != context.world_key {
            continue;
        }
        match effect {
            PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                NativeWeldHailChildActorState::Knockback(knockback),
            )) => {
                active_knockback_target_ids.insert(knockback.target_id.clone());
            }
            PrimarySpellTransient::Staff(
                crate::spells::staff::NativePlayerStaffTransient::ContactKnockback(knockback),
            ) => {
                active_knockback_target_ids.insert(knockback.target_id.clone());
            }
            _ => {}
        }
    }
    let mut work = ContactWork {
        enemies,
        rng,
        order,
        context,
        source,
        next_id,
        consumed_projectiles: BTreeSet::new(),
        consumed_transients: BTreeSet::new(),
        updated_projectiles: BTreeMap::new(),
        updated_transients: BTreeMap::new(),
        active_knockback_target_ids,
        impact_transients: Vec::new(),
        owned_transients: Vec::new(),
        pending_fire_contacts,
        result: BoneyardSpellCombatResult::default(),
    };
    if let Some(terminal) = special_death {
        work.resolve_special_death_payload(
            terminal.payload,
            terminal.victim_id,
            terminal.position,
            terminal.owner_id,
        );
    } else {
        work.resolve_hurricane_contacts();
        work.resolve_ether_blasts();
        work.resolve_weld_transient_forces();
        let frost_world_contacts = work.context.frost_missile_world_contacts;
        for projectile in frost_world_contacts {
            work.resolve_frost_missile_area_contact(projectile);
        }
        work.resolve_projectiles();
        work.resolve_weld_persistent_contacts();
        work.resolve_delayed_embers();
        work.resolve_weld_meteor_contacts();
        work.resolve_fire_contacts();
        work.resolve_channels();
    }
    let mut projectiles = work.source.projectiles;
    projectiles.retain_mut(|projectile| {
        let id = projectile.common().id;
        if work.consumed_projectiles.contains(&id) {
            return false;
        }
        if let Some(updated) = work.updated_projectiles.remove(&id) {
            projectile.replace(updated);
        }
        true
    });
    spells.projectiles = projectiles;
    for effect in work
        .source
        .transients
        .into_iter()
        .chain(work.impact_transients)
        .chain(work.owned_transients)
    {
        if !work.consumed_transients.contains(&effect.common().id) {
            spells
                .transients
                .push(work.updated_transients.remove(&effect.common().id).unwrap_or(effect));
        }
    }
    spells.next_id = work.next_id;
    work.result
}

impl ContactWork<'_, '_> {
    fn damage(
        &mut self,
        actor_id: usize,
        amount: f64,
        owner_id: &str,
        has_magic_damage: bool,
    ) -> EnemyDamageResult {
        self.damage_with_hurt_sound(actor_id, amount, owner_id, has_magic_damage, false)
    }

    fn damage_with_hurt_sound(
        &mut self,
        actor_id: usize,
        amount: f64,
        owner_id: &str,
        has_magic_damage: bool,
        suppress_hurt_sound: bool,
    ) -> EnemyDamageResult {
        self.damage_with_flags(
            actor_id,
            amount,
            owner_id,
            has_magic_damage,
            suppress_hurt_sound,
            false,
        )
    }

    fn damage_with_flags(
        &mut self,
        actor_id: usize,
        amount: f64,
        owner_id: &str,
        has_magic_damage: bool,
        suppress_hurt_sound: bool,
        suppress_hit_reaction: bool,
    ) -> EnemyDamageResult {
        let mut request =
            EnemyDamageRequest::new(actor_id, amount, self.context.tick, Some(owner_id.into()));
        request.magic = true;
        request.has_magic_damage = has_magic_damage;
        request.suppress_hurt_sound = suppress_hurt_sound;
        request.suppress_hit_reaction = suppress_hit_reaction;
        self.damage_with_request(request)
    }

    fn damage_with_request(&mut self, mut request: EnemyDamageRequest) -> EnemyDamageResult {
        request.enhanced_effects = Some(self.context.enhanced_effects);
        let actor_id = request.actor_id;
        let owner_id =
            request.source_player_id.clone().expect("primary damage owns its source player");
        let maximum_health = self
            .enemies
            .actors
            .iter()
            .find(|actor| actor.id == actor_id)
            .map(|actor| actor.config.maximum_health)
            .or_else(|| {
                self.enemies
                    .maggots
                    .iter()
                    .find(|maggot| maggot.id == actor_id)
                    .map(|maggot| maggot.maximum_health)
            });
        let result = self.enemies.damage(request, Some(self.order));
        if result.killed
            && let Some(actor) = self.enemies.actors.iter().find(|actor| actor.id == actor_id)
            && !matches!(actor.brain, crate::enemies::model::EnemyBrain::Cocoon(_))
        {
            // A true lethal hit preserves config. Armor conversion is accepted but
            // nonlethal; Cocoons and Maggots have no source onReward callback here.
            self.result.damage_notices.push(BoneyardSpellDamageNotice::LethalReward {
                actor_id,
                enemy: Box::new(actor.config.clone()),
                owner_id: owner_id.clone(),
            });
        }
        if result.health_damage > 0.0 {
            self.result.damage_notices.push(BoneyardSpellDamageNotice::HealthDamage {
                actor_id,
                amount: result.health_damage,
                maximum_health: maximum_health.expect("accepted enemy health damage has a target"),
                owner_id,
            });
        }
        result
    }

    fn multiplier(&self, actor_id: usize, kind: BoneyardSpellHitKind, owner_id: &str) -> f64 {
        let multiplier = (self.context.damage_multiplier)(actor_id, kind, owner_id);
        assert!(
            multiplier.is_finite() && multiplier >= 0.0,
            "Boneyard spell damage multiplier must be finite and nonnegative"
        );
        multiplier
    }

    fn hit(
        &mut self,
        actor_id: usize,
        amount: f64,
        owner_id: &str,
        spell_id: u64,
        kind: BoneyardSpellHitKind,
        killed: bool,
    ) {
        self.result.hits.push(BoneyardSpellHit {
            actor_id,
            amount,
            killed,
            owner_id: owner_id.into(),
            spell_id,
            spell_kind: kind,
            tick: self.context.tick,
        });
    }

    fn burn(&mut self, actor_id: usize, owner_id: &str, damage: f64) {
        if damage > 0.0 {
            self.result.burns.push(BoneyardSpellBurnContact {
                damage,
                owner_id: owner_id.into(),
                target_id: actor_id,
            });
        }
    }

    fn target_effect(&mut self, actor_id: usize, patch: NativeSecondaryTargetEffectPatch) {
        self.result.target_effects.push(BoneyardSpellTargetEffectContact {
            patch,
            target_id: actor_id,
            world_key: self.context.world_key.into(),
        });
    }

    fn publish_impact(&mut self, projectile: &PrimarySpellProjectile, origin: Vec2) {
        let mut projectile = projectile.clone();
        projectile.common_mut().position = origin;
        let effect = match &projectile {
            PrimarySpellProjectile::Weld(spell) => {
                let mut actor = NativeWeldWorldActor::Impact(create_primary_spell_weld_impact(
                    NativeWeldActorBirth {
                        build_id: spell.build_id,
                        direction: spell.common.direction,
                        id: self.next_id,
                        origin,
                        owner_id: &spell.common.owner_id,
                        tick: self.context.tick,
                        vector: &spell.vector,
                        world_key: &spell.common.world_key,
                    },
                    self.rng,
                ));
                enroll_native_weld_actor_painter(&mut actor, self.order);
                PrimarySpellTransient::Weld(actor)
            }
            PrimarySpellProjectile::Earth(spell) => {
                PrimarySpellTransient::EarthImpact(create_primary_spell_earth_impact(
                    self.next_id,
                    spell,
                    self.context.tick,
                    self.order,
                    self.context.enhanced_effects,
                ))
            }
            PrimarySpellProjectile::Ether(spell) => {
                PrimarySpellTransient::EtherImpact(create_primary_spell_ether_impact(
                    self.next_id,
                    spell,
                    self.context.tick,
                    self.order,
                ))
            }
            PrimarySpellProjectile::Fire(spell) => {
                PrimarySpellTransient::FireImpact(fire_impact_at(
                    self.next_id,
                    origin,
                    &spell.common.owner_id,
                    &spell.common.world_key,
                    self.order,
                ))
            }
        };
        self.impact_transients.push(effect);
        self.next_id += 1;
    }

    fn publish_earth_debris(&mut self, spell: &PrimarySpellEarthProjectile, charge: f64) {
        let debris = create_native_weld_boulder_contact_debris_program(charge, self.rng);
        let id = self.next_id;
        self.next_id += 1;
        // The source consumes the contact program and id before testing a 0/0 pool.
        if !charge.is_finite() {
            return;
        }
        self.impact_transients.push(PrimarySpellTransient::EarthBoulderBit(
            create_primary_spell_earth_boulder_bit(
                CreatePrimarySpellEarthBoulderBitInput {
                    debris,
                    enhanced_effects: true,
                    id,
                    origin: spell.common.position,
                    owner_id: &spell.common.owner_id,
                    tick: self.context.tick,
                    world_key: &spell.common.world_key,
                },
                self.order,
            ),
        ));
    }

    fn detonate_fire(&mut self, spell: &PrimarySpellFireProjectile) {
        let detonation = create_primary_spell_fire_detonation(
            self.next_id,
            spell,
            spell.common.position,
            self.rng,
            self.order,
        );
        self.next_id = detonation.next_id;
        self.pending_fire_contacts
            .extend(detonation.contacts.into_iter().map(FireActorContact::Ember));
        self.impact_transients.extend(detonation.transients);
    }

    fn resolve_ether_blasts(&mut self) {
        let mut effects: Vec<_> = self
            .source
            .transients
            .iter()
            .filter_map(|effect| {
                if let PrimarySpellTransient::EtherBlast(effect) = effect {
                    Some(effect.clone())
                } else {
                    None
                }
            })
            .collect();
        effects.sort_by_key(|effect| effect.common.id);
        for effect in effects {
            if effect.birth_tick != self.context.tick
                || effect.common.world_key != self.context.world_key
            {
                continue;
            }
            for row in root_target_rows(
                self.enemies,
                effect.origin,
                NATIVE_ETHER_BLAST_CONTACT_RADIUS,
                0x2,
            ) {
                let amount = native_ether_blast_damage(effect.charges, row.current_health);
                let contact = self.damage(row.actor_id, amount, &effect.common.owner_id, true);
                if !contact.accepted {
                    continue;
                }
                self.hit(
                    row.actor_id,
                    amount,
                    &effect.common.owner_id,
                    effect.common.id,
                    BoneyardSpellHitKind::EtherBlast,
                    contact.killed,
                );
                self.result.ether_burns.push(BoneyardSpellEtherBurnContact {
                    owner_id: effect.common.owner_id.clone(),
                    target_id: row.actor_id,
                });
            }
        }
    }

    fn resolve_projectiles(&mut self) {
        let mut projectiles: Vec<_> =
            self.source.projectiles.iter().map(|spell| spell.snapshot()).collect();
        projectiles.sort_by_key(|spell| spell.common().id);
        for projectile in projectiles {
            let common = projectile.common();
            if common.phase != PrimarySpellProjectilePhase::Flight
                || common.world_key != self.context.world_key
            {
                continue;
            }
            let rows = target_rows(self.enemies);
            if let PrimarySpellProjectile::Earth(spell) = &projectile {
                self.resolve_earth_contact(spell, &rows);
                continue;
            }
            if let PrimarySpellProjectile::Weld(spell) = &projectile {
                self.resolve_weld_projectile_contact(spell, &rows);
                continue;
            }
            let mut targets = self.context.primary_scenery_targets.to_vec();
            targets.extend(rows.iter().map(|row| row.target.clone()));
            let target = match &projectile {
                PrimarySpellProjectile::Fire(_) => {
                    first_native_fireball_point_contact(FireballPointContactQuery {
                        contact: PrimaryPointContactQuery {
                            actor_mask: 0x6,
                            position: common.position,
                            query_radius: 20.0,
                            targets: &targets,
                        },
                        direction: common.direction,
                        hostile_corridor_length: (self.context.fireball_hostile_corridor_length)(
                            &common.owner_id,
                        ),
                    })
                }
                PrimarySpellProjectile::Ether(spell) => {
                    first_native_primary_point_contact(PrimaryPointContactQuery {
                        actor_mask: if common.age_ticks >= 200 || spell.target_id.is_none() {
                            0x6
                        } else {
                            0x2
                        },
                        position: common.position,
                        query_radius: 6.0,
                        targets: &targets,
                    })
                }
                PrimarySpellProjectile::Earth(_) | PrimarySpellProjectile::Weld(_) => {
                    unreachable!("family contact already resolved")
                }
            };
            let Some(target) = target else {
                continue;
            };
            let Some(row) = rows.iter().find(|row| row.target.id == target.id) else {
                self.consumed_projectiles.insert(common.id);
                self.publish_impact(&projectile, common.position);
                continue;
            };
            if let PrimarySpellProjectile::Fire(spell) = &projectile {
                let amount = native_fire_direct_damage(common.damage, spell.explode_damage);
                if amount > 0.0 {
                    let damaged = self.damage(row.actor_id, amount, &common.owner_id, true);
                    if !damaged.accepted {
                        continue;
                    }
                    self.burn(row.actor_id, &common.owner_id, spell.burn_damage);
                    self.hit(
                        row.actor_id,
                        amount,
                        &common.owner_id,
                        common.id,
                        BoneyardSpellHitKind::Fire,
                        damaged.killed,
                    );
                }
                self.consumed_projectiles.insert(common.id);
                self.detonate_fire(spell);
                continue;
            }
            let amount = common.damage
                * self.multiplier(row.actor_id, BoneyardSpellHitKind::Ether, &common.owner_id);
            let damaged = self.damage(row.actor_id, amount, &common.owner_id, true);
            if !damaged.accepted {
                continue;
            }
            self.hit(
                row.actor_id,
                amount,
                &common.owner_id,
                common.id,
                BoneyardSpellHitKind::Ether,
                damaged.killed,
            );
            if let PrimarySpellProjectile::Ether(spell) = &projectile
                && spell.pierces_remaining > 0
            {
                let (spell, origins) = continue_piercing_ether(spell.clone(), target, &targets);
                for origin in origins {
                    let mut common = transient_base(
                        self.next_id,
                        &spell.common.owner_id,
                        &spell.common.world_key,
                        None,
                    );
                    common
                        .painter_registrations
                        .push(self.order.register(WorldManagerLane::Transient));
                    self.impact_transients.push(PrimarySpellTransient::EtherPierceStreak(
                        PrimarySpellEtherPierceStreak {
                            common,
                            heading_degrees: spell.heading_degrees,
                            origin,
                            visual_scale: spell.visual_scale,
                        },
                    ));
                    self.next_id += 1;
                }
                self.updated_projectiles.insert(common.id, PrimarySpellProjectile::Ether(spell));
            } else {
                self.consumed_projectiles.insert(common.id);
                self.publish_impact(&projectile, common.position);
            }
        }
    }

    fn resolve_earth_contact(&mut self, spell: &PrimarySpellEarthProjectile, rows: &[TargetRow]) {
        let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
        let contacts = native_primary_root_targets(
            spell.common.position,
            spell.common.charge * 75.0,
            0x6,
            &targets,
        );
        let contacts: Vec<_> = contacts
            .into_iter()
            .filter(|target| !spell.hit_target_ids.contains(&target.id))
            .collect();
        if contacts.is_empty() {
            return;
        }
        let mut updated = spell.clone();
        let mut charge = spell.common.charge;
        let mut continue_traversal = true;
        for target in contacts {
            if !continue_traversal {
                break;
            }
            let row =
                rows.iter().find(|row| row.target.id == target.id).expect("gathered enemy row");
            let contact = consume_native_earth_boulder_contact(NativeEarthBoulderContactInput {
                release_base_damage: spell.common.damage,
                release_charge: spell.maximum_charge,
                remaining_pool: updated.remaining_damage,
                target_health: row.current_health.max(0.0),
                toughness: spell.toughness,
            });
            let amount = contact.damage
                * self.multiplier(
                    row.actor_id,
                    BoneyardSpellHitKind::Earth,
                    &spell.common.owner_id,
                );
            let damaged = self.damage(row.actor_id, amount, &spell.common.owner_id, true);
            if !damaged.accepted {
                continue;
            }
            updated.hit_target_ids.push(target.id.clone());
            charge = contact.charge;
            continue_traversal = contact.continue_traversal;
            self.publish_earth_debris(spell, charge);
            updated.remaining_damage = contact.remaining_pool;
            self.hit(
                row.actor_id,
                amount,
                &spell.common.owner_id,
                spell.common.id,
                BoneyardSpellHitKind::Earth,
                damaged.killed,
            );
        }
        if updated.remaining_damage <= 0.0 {
            self.consumed_projectiles.insert(spell.common.id);
            self.publish_impact(
                &PrimarySpellProjectile::Earth(spell.clone()),
                spell.common.position,
            );
        } else {
            updated.common.charge = charge;
            updated.shell_charge = charge;
            self.updated_projectiles
                .insert(spell.common.id, PrimarySpellProjectile::Earth(updated));
        }
    }
}

impl ContactWork<'_, '_> {
    fn resolve_delayed_embers(&mut self) {
        let mut effects: Vec<_> = self
            .source
            .transients
            .iter()
            .filter_map(|effect| {
                if let PrimarySpellTransient::FireEmber(ember) = effect {
                    Some(ember.clone())
                } else {
                    None
                }
            })
            .collect();
        effects.sort_by_key(|effect| effect.common.id);
        for effect in effects {
            if !effect.contact_due || effect.common.world_key != self.context.world_key {
                continue;
            }
            let rows = target_rows(self.enemies);
            let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
            let Some(target) = first_native_primary_point_contact(PrimaryPointContactQuery {
                actor_mask: 0x2,
                position: effect.position,
                query_radius: 7.0,
                targets: &targets,
            }) else {
                continue;
            };
            let row =
                rows.iter().find(|row| row.target.id == target.id).expect("contacted enemy row");
            self.burn(row.actor_id, &effect.common.owner_id, effect.burn_damage);
            let damaged = self.damage(row.actor_id, effect.damage, &effect.common.owner_id, true);
            if !damaged.accepted {
                continue;
            }
            self.hit(
                row.actor_id,
                effect.damage,
                &effect.common.owner_id,
                effect.common.id,
                BoneyardSpellHitKind::FireEmber,
                damaged.killed,
            );
            self.consume_ember(
                effect.common.id,
                effect.position,
                &effect.common.owner_id,
                &effect.common.world_key,
            );
        }
    }

    fn consume_ember(&mut self, id: u64, position: Vec2, owner_id: &str, world_key: &str) {
        self.consumed_transients.insert(id);
        self.impact_transients.push(PrimarySpellTransient::FireImpact(fire_impact_at(
            self.next_id,
            position,
            owner_id,
            world_key,
            self.order,
        )));
        self.next_id += 1;
    }

    fn resolve_channels(&mut self) {
        let mut emissions = self.context.channel_emissions.to_vec();
        emissions.sort_by_key(|emission| emission.id);
        for emission in emissions {
            if emission.world_key != self.context.world_key {
                continue;
            }
            match emission.kind {
                PrimarySpellChannelKind::Air => self.resolve_air_channel(&emission),
                PrimarySpellChannelKind::Water => self.resolve_water_channel(&emission),
                PrimarySpellChannelKind::Weld => self.resolve_weld_channel(&emission),
            }
        }
    }

    fn resolve_air_channel(&mut self, emission: &PrimarySpellChannelEmission) {
        let NativePrimarySkillProfile::Air(profile) = &emission.primary_skill else {
            panic!("Air channel requires an Air profile");
        };
        let mut target_id = self.source.transients.iter().find_map(|effect| {
            if let PrimarySpellTransient::Air(effect) = effect
                && effect.common.id == emission.id
                && effect.common.owner_id == emission.owner_id
                && effect.common.world_key == emission.world_key
            {
                effect.target_id.clone()
            } else {
                None
            }
        });
        let mut contacted_ids = BTreeSet::new();
        let mut damage = emission.damage;
        let mut previous_point = emission.origin;
        let mut hop = 0;
        while let Some(id) = target_id {
            let rows = target_rows(self.enemies);
            let Some(row) = rows.iter().find(|row| {
                row.target.id == id && native_primary_target_eligible(&row.target, 0x2)
            }) else {
                break;
            };
            contacted_ids.insert(id);
            if !emission.underpowered && profile.stun_movement_factor < 1.0 {
                self.target_effect(
                    row.actor_id,
                    NativeSecondaryTargetEffectPatch {
                        stun_factor: Some(profile.stun_movement_factor),
                        stun_ticks: Some(25),
                        ..NativeSecondaryTargetEffectPatch::default()
                    },
                );
            }
            let disintegrate = !emission.underpowered
                && profile.disintegrate_chance > 0.0
                && self.context.tick % 40 == row.disintegrate_phase as u64 % 40
                && draw_native_disintegrate_percentile(self.rng, profile.disintegrate_chance);
            // Native Lightning samples this visual scalar after its optional execute roll.
            self.rng.draw_scaled_float(0.5, false);
            let amount = damage
                * self.multiplier(row.actor_id, BoneyardSpellHitKind::Air, &emission.owner_id);
            let (accepted, amount, killed) = self.damage_with_disintegrate(
                row.actor_id,
                amount,
                &emission.owner_id,
                disintegrate,
            );
            if accepted {
                self.hit(
                    row.actor_id,
                    amount,
                    &emission.owner_id,
                    emission.id,
                    BoneyardSpellHitKind::Air,
                    killed,
                );
            }
            let point = primary_spell_target_point(&row.target);
            if hop > 0 {
                let direction = normalized_difference(previous_point, point);
                let geometry = air_primary_bolt_geometry(previous_point, direction, point);
                let light = self.order.register(WorldManagerLane::Transient);
                let mut common = transient_base(
                    self.next_id,
                    &emission.owner_id,
                    self.context.world_key,
                    Some(light),
                );
                common.painter_registrations = self.order.register_many(WorldManagerLane::Actor, 3);
                self.owned_transients.push(PrimarySpellTransient::Air(PrimarySpellAirTransient {
                    common,
                    chained: true,
                    enhanced_effects: self.context.enhanced_effects,
                    birth_tick: self.context.tick,
                    direction,
                    endpoint: geometry.endpoint,
                    hurricane_charge: 0.0,
                    midpoint: geometry.midpoint,
                    origin: geometry.source,
                    target_id: Some(row.target.id.clone()),
                    underpowered: emission.underpowered,
                    variant: (self.next_id % 4) as u32,
                }));
                self.next_id += 1;
            }
            previous_point = point;
            damage = fround(damage * 0.6_f32 as f64);
            target_id = if emission.underpowered || hop >= profile.arc_count {
                None
            } else {
                nearest_unused_air_chain_target(
                    self.enemies,
                    row.target.position,
                    &contacted_ids,
                    200.0,
                )
            };
            hop += 1;
        }
    }

    fn damage_with_disintegrate(
        &mut self,
        actor_id: usize,
        amount: f64,
        owner_id: &str,
        disintegrate: bool,
    ) -> (bool, f64, bool) {
        let ordinary = self.damage(actor_id, amount, owner_id, true);
        if !ordinary.accepted || ordinary.killed || !disintegrate {
            return (ordinary.accepted, amount, ordinary.killed);
        }
        let Some(row) = target_rows(self.enemies).into_iter().find(|row| row.actor_id == actor_id)
        else {
            return (true, amount, false);
        };
        if row.current_health >= row.maximum_health * 0.2 || row.current_health <= 0.0 {
            return (true, amount, false);
        }
        let executed = self.damage(actor_id, row.current_health, owner_id, true);
        (
            true,
            amount + if executed.accepted { row.current_health } else { 0.0 },
            executed.accepted && executed.killed,
        )
    }

    fn resolve_water_channel(&mut self, emission: &PrimarySpellChannelEmission) {
        let NativePrimarySkillProfile::Water(profile) = &emission.primary_skill else {
            panic!("Water channel requires a Water profile");
        };
        let pushback = if emission.underpowered { 0.0 } else { profile.pushback_factor };
        let rows = water_target_rows(self.enemies);
        let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
        let has_line_of_sight = |target: &PrimarySpellTarget| {
            self.context.first_world_contact.is_none_or(|contact| {
                contact(emission.query_origin, target.position, 0.0).is_none()
            })
        };
        let contacts = native_primary_cone_targets(PrimaryConeQuery {
            actor_mask: if emission.underpowered { 0x2 } else { 0x1082 },
            aim_direction: emission.direction,
            half_angle_degrees: if emission.underpowered {
                15.0
            } else {
                profile.half_angle_degrees
            },
            has_line_of_sight: &has_line_of_sight,
            origin: emission.query_origin,
            reach: if emission.underpowered { 205.0 } else { profile.reach },
            targets: &targets,
        });
        // Keep the source query snapshot while mutation updates the authoritative store.
        let contacts: Vec<_> = contacts.into_iter().cloned().collect();
        for target in contacts {
            let row =
                rows.iter().find(|row| row.target.id == target.id).expect("queried Water row");
            if self.apply_channel_projectile_force(
                row,
                fround(pushback * 0.32_f32 as f64),
                emission.direction,
            ) {
                continue;
            }
            let ChannelTargetKind::Enemy { actor_id } = row.kind else { continue };
            self.target_effect(
                actor_id,
                NativeSecondaryTargetEffectPatch {
                    cold_slow_factor: Some(if emission.underpowered {
                        0.75
                    } else {
                        profile.cold_movement_factor
                    }),
                    cold_slow_material: Some(true),
                    cold_slow_ticks: Some(if emission.underpowered {
                        25
                    } else {
                        profile.cold_duration_ticks
                    }),
                    ..NativeSecondaryTargetEffectPatch::default()
                },
            );
            let amount = emission.damage
                * self.multiplier(actor_id, BoneyardSpellHitKind::Water, &emission.owner_id);
            let damaged =
                self.damage_with_flags(actor_id, amount, &emission.owner_id, true, false, true);
            if damaged.accepted {
                self.hit(
                    actor_id,
                    amount,
                    &emission.owner_id,
                    emission.id,
                    BoneyardSpellHitKind::Water,
                    damaged.killed,
                );
                if !emission.underpowered
                    && profile.hail_threshold > 0
                    && (self.rng.draw_integer(3_000) as u32) < profile.hail_threshold
                {
                    let hail_damage = draw_native_spell_damage(
                        self.rng,
                        profile.hail_damage_minimum,
                        profile.hail_damage_maximum,
                    );
                    let amount = hail_damage
                        * self.multiplier(
                            actor_id,
                            BoneyardSpellHitKind::WaterHail,
                            &emission.owner_id,
                        );
                    let hail = self.damage(actor_id, amount, &emission.owner_id, false);
                    if hail.accepted {
                        self.hit(
                            actor_id,
                            amount,
                            &emission.owner_id,
                            emission.id,
                            BoneyardSpellHitKind::WaterHail,
                            hail.killed,
                        );
                    }
                }
            }
            if pushback > 0.0
                && let Some(live) =
                    target_rows(self.enemies).into_iter().find(|row| row.actor_id == actor_id)
                && !frost_jet_push_blocked(self.enemies, &live.target, emission.origin)
            {
                let target_factor =
                    if target.actor_flags & 0x40 != 0 { 0.1_f32 as f64 } else { 1.0 };
                self.apply_water_pushback(
                    live.actor_id,
                    &live.target,
                    emission.origin,
                    pushback,
                    profile.reach,
                    target_factor,
                );
            }
        }
        if !emission.underpowered
            && profile.aura_radius_scale > 0.0
            && self.context.tick.is_multiple_of(crate::cold_aura::NATIVE_COLD_AURA_PERIOD_TICKS)
        {
            for row in root_target_rows(
                self.enemies,
                emission.query_origin,
                crate::cold_aura::native_cold_aura_query_radius(profile.aura_radius_scale),
                0x2,
            ) {
                self.target_effect(
                    row.actor_id,
                    NativeSecondaryTargetEffectPatch {
                        cold_slow_factor: Some(profile.aura_movement_factor),
                        cold_slow_material: Some(true),
                        cold_slow_ticks: Some(profile.cold_duration_ticks),
                        ..NativeSecondaryTargetEffectPatch::default()
                    },
                );
            }
        }
    }

    fn apply_water_pushback(
        &mut self,
        actor_id: usize,
        target: &PrimarySpellTarget,
        origin: Vec2,
        pushback: f64,
        reach: f64,
        target_factor: f64,
    ) {
        let squared = fround(squared_distance(origin, target.position));
        let base_radius = fround(reach - 25.0);
        let outer_squared = fround(fround(base_radius * base_radius) * 0.75);
        if squared >= outer_squared {
            return;
        }
        let inner_squared = fround(0.5 * outer_squared);
        let attenuation = if squared <= inner_squared {
            1.0
        } else {
            fround((outer_squared - squared) / (outer_squared - inner_squared))
        };
        let distance = fround(squared.sqrt());
        if distance == 0.0 {
            return;
        }
        let magnitude = fround(fround(fround(pushback * target_factor) * 2.5) * attenuation);
        let direction = Vec2::new(
            fround((target.position.x - origin.x) / distance),
            fround((target.position.y - origin.y) / distance),
        );
        let requested = Vec2::new(
            fround(target.position.x + fround(direction.x * magnitude)),
            fround(target.position.y + fround(direction.y * magnitude)),
        );
        let resolved = (self.context.resolve_enemy_movement)(
            actor_id,
            target.position,
            requested,
            target.body_radius,
        );
        self.enemies.position_enemy(actor_id, resolved);
    }
}

fn fire_contact_id(contact: &FireActorContact) -> u64 {
    match contact {
        FireActorContact::Ember(contact) | FireActorContact::Patch(contact) => contact.spell_id,
        FireActorContact::GoodImp(contact) => contact.spell_id,
    }
}

fn normalized_difference(origin: Vec2, target: Vec2) -> Vec2 {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let length = hypot(dx, dy);
    if length == 0.0 { Vec2::new(0.0, -1.0) } else { Vec2::new(dx / length, dy / length) }
}

fn nearest_unused_air_chain_target(
    enemies: &EnemyWorld,
    origin: Vec2,
    contacted: &BTreeSet<String>,
    radius: f64,
) -> Option<String> {
    let targets = enemies.spell_targets();
    let mut selected = None;
    let mut selected_distance = f64::INFINITY;
    for target in native_primary_root_targets(origin, radius, 0x2, &targets) {
        if contacted.contains(&target.id) {
            continue;
        }
        let distance = squared_distance(origin, target.position);
        if distance < selected_distance {
            selected = Some(target.id.clone());
            selected_distance = distance;
        }
    }
    selected
}

#[derive(Clone, Copy)]
enum ChannelTargetKind {
    Enemy { actor_id: usize },
    Arrow { id: usize, accumulator: f64 },
    Silk { id: usize },
    Scenery,
}
struct ChannelTargetRow {
    target: PrimarySpellTarget,
    kind: ChannelTargetKind,
}

fn water_target_rows(enemies: &EnemyWorld) -> Vec<ChannelTargetRow> {
    let mut rows: Vec<_> = target_rows(enemies)
        .into_iter()
        .map(|row| ChannelTargetRow {
            target: row.target,
            kind: ChannelTargetKind::Enemy { actor_id: row.actor_id },
        })
        .collect();
    for target in enemies.projectile_force_targets() {
        let kind = if let Some(id) = target.id.strip_prefix("projectile:") {
            let id = id.parse().expect("canonical projectile target id");
            let projectile = enemies
                .projectiles
                .iter()
                .find(|projectile| {
                    projectile.state().id == id && projectile.kind() == EnemyProjectileKind::Arrow
                })
                .expect("queried Arrow");
            ChannelTargetKind::Arrow {
                id,
                accumulator: projectile.state().chill_tumble_accumulator,
            }
        } else {
            let id = target
                .id
                .strip_prefix("silk:")
                .expect("canonical Silk target id")
                .parse()
                .expect("numeric Silk target id");
            ChannelTargetKind::Silk { id }
        };
        rows.push(ChannelTargetRow { target, kind });
    }
    rows
}

fn frost_jet_push_blocked(enemies: &EnemyWorld, target: &PrimarySpellTarget, origin: Vec2) -> bool {
    let squared = fround(squared_distance(origin, target.position));
    if squared <= 0.0 {
        return false;
    }
    let distance = fround(squared.sqrt());
    let direction = Vec2::new(
        fround((target.position.x - origin.x) / distance),
        fround((target.position.y - origin.y) / distance),
    );
    let probe = Vec2::new(
        fround(target.position.x + fround(direction.x * 25.5)),
        fround(target.position.y + fround(direction.y * 25.5)),
    );
    let targets: Vec<_> =
        enemies.spell_targets().into_iter().filter(|candidate| candidate.id != target.id).collect();
    first_native_primary_point_contact(PrimaryPointContactQuery {
        actor_mask: 0x2,
        position: probe,
        query_radius: fround(25.0 / 3.0),
        targets: &targets,
    })
    .is_some()
}

fn target_rows(enemies: &EnemyWorld) -> Vec<TargetRow> {
    let mut rows = Vec::new();
    for (index, target) in enemies.spell_targets().into_iter().enumerate() {
        let (actor_id, current_health, maximum_health) = if index < enemies.actors.len() {
            let actor = enemies.actors.get(index).expect("enemy index is live");
            (actor.id, actor.current_health, actor.config.maximum_health)
        } else {
            let actor = enemies
                .maggots
                .get(index - enemies.actors.len())
                .expect("Maggot target index is live");
            (actor.id, actor.current_health, actor.maximum_health)
        };
        rows.push(TargetRow {
            actor_id,
            current_health,
            maximum_health,
            disintegrate_phase: index,
            target,
        });
    }
    rows
}

fn root_target_rows(enemies: &EnemyWorld, origin: Vec2, reach: f64, mask: u32) -> Vec<TargetRow> {
    let rows = target_rows(enemies);
    let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
    let ids: BTreeSet<_> = native_primary_root_targets(origin, reach, mask, &targets)
        .into_iter()
        .map(|target| target.id.as_str())
        .collect();
    // The source gather selects membership, then resumes the store's actor order.
    rows.into_iter().filter(|row| ids.contains(row.target.id.as_str())).collect()
}

fn continue_piercing_ether(
    mut spell: PrimarySpellEtherProjectile,
    contacted: &PrimarySpellTarget,
    targets: &[PrimarySpellTarget],
) -> (PrimarySpellEtherProjectile, Vec<Vec2>) {
    let mut origins = Vec::new();
    loop {
        spell.common.position = Vec2::new(
            fround(spell.common.position.x + spell.common.direction.x * 5.0),
            fround(spell.common.position.y + spell.common.direction.y * 5.0),
        );
        origins.push(spell.common.position);
        let radius = 6.0 + contacted.body_radius;
        if squared_distance(contacted.position, spell.common.position) >= radius * radius {
            break;
        }
    }
    let target =
        select_ether_primary_target_at_point(spell.common.position, Some(&contacted.id), targets)
            .or_else(|| select_ether_primary_target_at_point(spell.common.position, None, targets));
    spell.common.damage *= spell.damage_retention;
    spell.pierces_remaining -= 1;
    if target.is_none() {
        spell.reacquires_target = false;
    }
    spell.target_id = target.map(|target| target.id.clone());
    spell.visual_scale *= spell.damage_retention;
    (spell, origins)
}

fn squared_distance(left: Vec2, right: Vec2) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    dx * dx + dy * dy
}
