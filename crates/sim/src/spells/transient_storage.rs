//! The mixed effect order is independent of each concrete component's table.
//! Short fades therefore do not reserve storage for the largest persistent spell.

mod views;
pub use views::*;

use crate::ecs::{EntityStep, OrderedEntities};
use crate::harden_effects::NativeHardenEffect;
use crate::spells::model::*;
use crate::spells::staff::*;
use crate::spells::weld::*;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::{EntityMut, EntityRef, EntityWorldMut, World};
use std::fmt;

#[derive(Clone, Copy, bevy_ecs::component::Component)]
enum TransientKind {
    AirHurricane,
    WaterAura,
    WaterHail,
    Harden,
    StaffActionMelee,
    StaffActionSpin,
    StaffContact,
    StaffContactKnockback,
    StaffPikeBreak,
    StaffVfxSmoke,
    StaffVfxMoveFade,
    StaffVfxPerspectiveFade,
    StaffKnockback,
    WeldBlizzardChainFrost,
    WeldBlizzardGlow,
    WeldBoulderDebris,
    WeldChannel,
    WeldFlameLashFade,
    WeldFrostFade,
    WeldGroundSparkFade,
    WeldHailChildFlash,
    WeldHailChildKnockback,
    WeldHailChildLine,
    WeldHailChildTerrainBouncer,
    WeldHailChildTerrainParticle,
    WeldHailRockFade,
    WeldImpact,
    WeldMeteor,
    WeldMeteorFlash,
    WeldMeteorMarker,
    WeldPersistentEtherealBoulder,
    WeldPersistentHailstones,
    WeldPersistentMeteorField,
    WeldSteam,
    EarthBoulderBit,
    Air,
    Water,
    EarthImpact,
    EarthCalledRock,
    EtherImpact,
    EtherPierceStreak,
    EtherBlast,
    FireParticle,
    FireImpact,
    FireEmber,
    FireExplosion,
    FireGoodImp,
    FirePatch,
}

#[derive(Default)]
pub struct PrimaryTransients {
    entities: OrderedEntities<EntityMut<'static>>,
    births: Option<Vec<PrimarySpellTransient>>,
}
impl PrimaryTransients {
    pub fn len(&self) -> usize {
        self.entities.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = PrimaryTransientRef<'_>> + ExactSizeIterator + Clone {
        self.entities.iter().map(Self::read)
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = PrimaryTransientMut<'_>> {
        self.entities.iter_mut().map(Self::write)
    }
    fn read(entity: EntityRef<'_>) -> PrimaryTransientRef<'_> {
        match *entity.get::<TransientKind>().expect("transient owns its kind") {
            TransientKind::AirHurricane => PrimaryTransientRef::AirHurricane(
                entity.get::<PrimarySpellAirHurricane>().expect("transient owns its component"),
            ),
            TransientKind::WaterAura => PrimaryTransientRef::WaterAura(
                entity.get::<PrimarySpellWaterAura>().expect("transient owns its component"),
            ),
            TransientKind::WaterHail => PrimaryTransientRef::WaterHail(
                entity.get::<PrimarySpellWaterHail>().expect("transient owns its component"),
            ),
            TransientKind::Harden => PrimaryTransientRef::Harden(
                entity.get::<NativeHardenEffect>().expect("transient owns its component"),
            ),
            TransientKind::StaffActionMelee => {
                PrimaryTransientRef::Staff(StaffTransientRef::Action(StaffActionRef::Melee(
                    entity
                        .get::<NativePlayerStaffMeleeAction>()
                        .expect("transient owns its component"),
                )))
            }
            TransientKind::StaffActionSpin => {
                PrimaryTransientRef::Staff(StaffTransientRef::Action(StaffActionRef::Spin(
                    entity
                        .get::<NativePlayerStaffSpinAction>()
                        .expect("transient owns its component"),
                )))
            }
            TransientKind::StaffContact => PrimaryTransientRef::Staff(StaffTransientRef::Contact(
                entity
                    .get::<NativePlayerStaffContactEvent>()
                    .expect("transient owns its component"),
            )),
            TransientKind::StaffContactKnockback => {
                PrimaryTransientRef::Staff(StaffTransientRef::ContactKnockback(
                    entity
                        .get::<NativePlayerStaffContactKnockback>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::StaffPikeBreak => {
                PrimaryTransientRef::Staff(StaffTransientRef::PikeBreak(
                    entity
                        .get::<NativePlayerStaffPikeBreakVfx>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::StaffVfxSmoke => {
                PrimaryTransientRef::Staff(StaffTransientRef::Vfx(StaffVfxRef::Smoke(
                    entity
                        .get::<NativePlayerStaffSmokeVfx>()
                        .expect("transient owns its component"),
                )))
            }
            TransientKind::StaffVfxMoveFade => {
                PrimaryTransientRef::Staff(StaffTransientRef::Vfx(StaffVfxRef::MoveFade(
                    entity
                        .get::<NativePlayerStaffMoveFadeVfx>()
                        .expect("transient owns its component"),
                )))
            }
            TransientKind::StaffVfxPerspectiveFade => {
                PrimaryTransientRef::Staff(StaffTransientRef::Vfx(StaffVfxRef::PerspectiveFade(
                    entity
                        .get::<NativePlayerStaffPerspectiveVfx>()
                        .expect("transient owns its component"),
                )))
            }
            TransientKind::StaffKnockback => {
                PrimaryTransientRef::Staff(StaffTransientRef::Knockback(
                    entity
                        .get::<NativeStaffKnockbackActor>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldBlizzardChainFrost => {
                PrimaryTransientRef::Weld(WeldWorldRef::BlizzardChainFrost(
                    entity
                        .get::<NativeWeldBlizzardChainFrostState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldBlizzardGlow => {
                PrimaryTransientRef::Weld(WeldWorldRef::BlizzardGlow(
                    entity
                        .get::<NativeWeldBlizzardGlowState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldBoulderDebris => {
                PrimaryTransientRef::Weld(WeldWorldRef::BoulderDebris(
                    entity
                        .get::<NativeWeldBoulderDebrisActorState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldChannel => PrimaryTransientRef::Weld(WeldWorldRef::Channel(
                entity.get::<NativeWeldChannelActorState>().expect("transient owns its component"),
            )),
            TransientKind::WeldFlameLashFade => {
                PrimaryTransientRef::Weld(WeldWorldRef::FlameLashFade(
                    entity
                        .get::<NativeWeldFlameLashFadeState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldFrostFade => PrimaryTransientRef::Weld(WeldWorldRef::FrostFade(
                entity
                    .get::<NativeWeldFrostFadeActorState>()
                    .expect("transient owns its component"),
            )),
            TransientKind::WeldGroundSparkFade => {
                PrimaryTransientRef::Weld(WeldWorldRef::GroundSparkFade(
                    entity
                        .get::<NativeWeldGroundSparkFadeActorState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldHailChildFlash => {
                PrimaryTransientRef::Weld(WeldWorldRef::HailChild(WeldHailChildRef::Flash(
                    entity.get::<NativeWeldHailFlashState>().expect("transient owns its component"),
                )))
            }
            TransientKind::WeldHailChildKnockback => {
                PrimaryTransientRef::Weld(WeldWorldRef::HailChild(WeldHailChildRef::Knockback(
                    entity
                        .get::<NativeWeldHailKnockbackState>()
                        .expect("transient owns its component"),
                )))
            }
            TransientKind::WeldHailChildLine => {
                PrimaryTransientRef::Weld(WeldWorldRef::HailChild(WeldHailChildRef::Line(
                    entity.get::<NativeWeldHailLineState>().expect("transient owns its component"),
                )))
            }
            TransientKind::WeldHailChildTerrainBouncer => PrimaryTransientRef::Weld(
                WeldWorldRef::HailChild(WeldHailChildRef::TerrainBouncer(
                    entity
                        .get::<NativeWeldHailTerrainBouncerState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldHailChildTerrainParticle => PrimaryTransientRef::Weld(
                WeldWorldRef::HailChild(WeldHailChildRef::TerrainParticle(
                    entity
                        .get::<NativeWeldHailTerrainParticleState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldHailRockFade => {
                PrimaryTransientRef::Weld(WeldWorldRef::HailRockFade(
                    entity
                        .get::<NativeWeldHailRockFadeActorState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldImpact => PrimaryTransientRef::Weld(WeldWorldRef::Impact(
                entity.get::<NativeWeldImpactActorState>().expect("transient owns its component"),
            )),
            TransientKind::WeldMeteor => PrimaryTransientRef::Weld(WeldWorldRef::Meteor(
                entity.get::<NativeWeldMeteorActorState>().expect("transient owns its component"),
            )),
            TransientKind::WeldMeteorFlash => PrimaryTransientRef::Weld(WeldWorldRef::MeteorFlash(
                entity
                    .get::<NativeWeldMeteorFlashActorState>()
                    .expect("transient owns its component"),
            )),
            TransientKind::WeldMeteorMarker => {
                PrimaryTransientRef::Weld(WeldWorldRef::MeteorMarker(
                    entity
                        .get::<NativeWeldMeteorMarkerState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldPersistentEtherealBoulder => PrimaryTransientRef::Weld(
                WeldWorldRef::Persistent(WeldPersistentRef::EtherealBoulder(
                    entity
                        .get::<NativeWeldEtherealBoulderState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldPersistentHailstones => {
                PrimaryTransientRef::Weld(WeldWorldRef::Persistent(WeldPersistentRef::Hailstones(
                    entity
                        .get::<NativeWeldHailstonesState>()
                        .expect("transient owns its component"),
                )))
            }
            TransientKind::WeldPersistentMeteorField => {
                PrimaryTransientRef::Weld(WeldWorldRef::Persistent(WeldPersistentRef::MeteorField(
                    entity
                        .get::<NativeWeldMeteorFieldState>()
                        .expect("transient owns its component"),
                )))
            }
            TransientKind::WeldSteam => PrimaryTransientRef::Weld(WeldWorldRef::Steam(
                entity.get::<NativeWeldSteamActorState>().expect("transient owns its component"),
            )),
            TransientKind::EarthBoulderBit => PrimaryTransientRef::EarthBoulderBit(
                entity.get::<PrimarySpellEarthBoulderBit>().expect("transient owns its component"),
            ),
            TransientKind::Air => PrimaryTransientRef::Air(
                entity.get::<PrimarySpellAirTransient>().expect("transient owns its component"),
            ),
            TransientKind::Water => PrimaryTransientRef::Water(
                entity.get::<PrimarySpellWaterTransient>().expect("transient owns its component"),
            ),
            TransientKind::EarthImpact => PrimaryTransientRef::EarthImpact(
                entity.get::<PrimarySpellEarthImpact>().expect("transient owns its component"),
            ),
            TransientKind::EarthCalledRock => PrimaryTransientRef::EarthCalledRock(
                entity.get::<PrimarySpellEarthCalledRock>().expect("transient owns its component"),
            ),
            TransientKind::EtherImpact => PrimaryTransientRef::EtherImpact(
                entity.get::<PrimarySpellEtherImpact>().expect("transient owns its component"),
            ),
            TransientKind::EtherPierceStreak => PrimaryTransientRef::EtherPierceStreak(
                entity
                    .get::<PrimarySpellEtherPierceStreak>()
                    .expect("transient owns its component"),
            ),
            TransientKind::EtherBlast => PrimaryTransientRef::EtherBlast(
                entity.get::<PrimarySpellEtherBlast>().expect("transient owns its component"),
            ),
            TransientKind::FireParticle => PrimaryTransientRef::FireParticle(
                entity.get::<PrimarySpellFireParticle>().expect("transient owns its component"),
            ),
            TransientKind::FireImpact => PrimaryTransientRef::FireImpact(
                entity.get::<PrimarySpellFireImpact>().expect("transient owns its component"),
            ),
            TransientKind::FireEmber => PrimaryTransientRef::FireEmber(
                entity.get::<PrimarySpellFireEmber>().expect("transient owns its component"),
            ),
            TransientKind::FireExplosion => PrimaryTransientRef::FireExplosion(
                entity.get::<PrimarySpellFireExplosion>().expect("transient owns its component"),
            ),
            TransientKind::FireGoodImp => PrimaryTransientRef::FireGoodImp(
                entity.get::<PrimarySpellFireGoodImp>().expect("transient owns its component"),
            ),
            TransientKind::FirePatch => PrimaryTransientRef::FirePatch(
                entity.get::<PrimarySpellFirePatch>().expect("transient owns its component"),
            ),
        }
    }
    fn write(entity: EntityMut<'_>) -> PrimaryTransientMut<'_> {
        match *entity.get::<TransientKind>().expect("transient owns its kind") {
            TransientKind::AirHurricane => PrimaryTransientMut::AirHurricane(
                entity
                    .into_mut::<PrimarySpellAirHurricane>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::WaterAura => PrimaryTransientMut::WaterAura(
                entity
                    .into_mut::<PrimarySpellWaterAura>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::WaterHail => PrimaryTransientMut::WaterHail(
                entity
                    .into_mut::<PrimarySpellWaterHail>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::Harden => PrimaryTransientMut::Harden(
                entity
                    .into_mut::<NativeHardenEffect>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::StaffActionMelee => {
                PrimaryTransientMut::Staff(StaffTransientMut::Action(StaffActionMut::Melee(
                    entity
                        .into_mut::<NativePlayerStaffMeleeAction>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::StaffActionSpin => {
                PrimaryTransientMut::Staff(StaffTransientMut::Action(StaffActionMut::Spin(
                    entity
                        .into_mut::<NativePlayerStaffSpinAction>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::StaffContact => PrimaryTransientMut::Staff(StaffTransientMut::Contact(
                entity
                    .into_mut::<NativePlayerStaffContactEvent>()
                    .expect("transient owns its component")
                    .into_inner(),
            )),
            TransientKind::StaffContactKnockback => {
                PrimaryTransientMut::Staff(StaffTransientMut::ContactKnockback(
                    entity
                        .into_mut::<NativePlayerStaffContactKnockback>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::StaffPikeBreak => {
                PrimaryTransientMut::Staff(StaffTransientMut::PikeBreak(
                    entity
                        .into_mut::<NativePlayerStaffPikeBreakVfx>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::StaffVfxSmoke => {
                PrimaryTransientMut::Staff(StaffTransientMut::Vfx(StaffVfxMut::Smoke(
                    entity
                        .into_mut::<NativePlayerStaffSmokeVfx>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::StaffVfxMoveFade => {
                PrimaryTransientMut::Staff(StaffTransientMut::Vfx(StaffVfxMut::MoveFade(
                    entity
                        .into_mut::<NativePlayerStaffMoveFadeVfx>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::StaffVfxPerspectiveFade => {
                PrimaryTransientMut::Staff(StaffTransientMut::Vfx(StaffVfxMut::PerspectiveFade(
                    entity
                        .into_mut::<NativePlayerStaffPerspectiveVfx>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::StaffKnockback => {
                PrimaryTransientMut::Staff(StaffTransientMut::Knockback(
                    entity
                        .into_mut::<NativeStaffKnockbackActor>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::WeldBlizzardChainFrost => {
                PrimaryTransientMut::Weld(WeldWorldMut::BlizzardChainFrost(
                    entity
                        .into_mut::<NativeWeldBlizzardChainFrostState>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::WeldBlizzardGlow => {
                PrimaryTransientMut::Weld(WeldWorldMut::BlizzardGlow(
                    entity
                        .into_mut::<NativeWeldBlizzardGlowState>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::WeldBoulderDebris => {
                PrimaryTransientMut::Weld(WeldWorldMut::BoulderDebris(
                    entity
                        .into_mut::<NativeWeldBoulderDebrisActorState>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::WeldChannel => PrimaryTransientMut::Weld(WeldWorldMut::Channel(
                entity
                    .into_mut::<NativeWeldChannelActorState>()
                    .expect("transient owns its component")
                    .into_inner(),
            )),
            TransientKind::WeldFlameLashFade => {
                PrimaryTransientMut::Weld(WeldWorldMut::FlameLashFade(
                    entity
                        .into_mut::<NativeWeldFlameLashFadeState>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::WeldFrostFade => PrimaryTransientMut::Weld(WeldWorldMut::FrostFade(
                entity
                    .into_mut::<NativeWeldFrostFadeActorState>()
                    .expect("transient owns its component")
                    .into_inner(),
            )),
            TransientKind::WeldGroundSparkFade => {
                PrimaryTransientMut::Weld(WeldWorldMut::GroundSparkFade(
                    entity
                        .into_mut::<NativeWeldGroundSparkFadeActorState>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::WeldHailChildFlash => {
                PrimaryTransientMut::Weld(WeldWorldMut::HailChild(WeldHailChildMut::Flash(
                    entity
                        .into_mut::<NativeWeldHailFlashState>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::WeldHailChildKnockback => {
                PrimaryTransientMut::Weld(WeldWorldMut::HailChild(WeldHailChildMut::Knockback(
                    entity
                        .into_mut::<NativeWeldHailKnockbackState>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::WeldHailChildLine => {
                PrimaryTransientMut::Weld(WeldWorldMut::HailChild(WeldHailChildMut::Line(
                    entity
                        .into_mut::<NativeWeldHailLineState>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::WeldHailChildTerrainBouncer => PrimaryTransientMut::Weld(
                WeldWorldMut::HailChild(WeldHailChildMut::TerrainBouncer(
                    entity
                        .into_mut::<NativeWeldHailTerrainBouncerState>()
                        .expect("transient owns its component")
                        .into_inner(),
                )),
            ),
            TransientKind::WeldHailChildTerrainParticle => PrimaryTransientMut::Weld(
                WeldWorldMut::HailChild(WeldHailChildMut::TerrainParticle(
                    entity
                        .into_mut::<NativeWeldHailTerrainParticleState>()
                        .expect("transient owns its component")
                        .into_inner(),
                )),
            ),
            TransientKind::WeldHailRockFade => {
                PrimaryTransientMut::Weld(WeldWorldMut::HailRockFade(
                    entity
                        .into_mut::<NativeWeldHailRockFadeActorState>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::WeldImpact => PrimaryTransientMut::Weld(WeldWorldMut::Impact(
                entity
                    .into_mut::<NativeWeldImpactActorState>()
                    .expect("transient owns its component")
                    .into_inner(),
            )),
            TransientKind::WeldMeteor => PrimaryTransientMut::Weld(WeldWorldMut::Meteor(
                entity
                    .into_mut::<NativeWeldMeteorActorState>()
                    .expect("transient owns its component")
                    .into_inner(),
            )),
            TransientKind::WeldMeteorFlash => PrimaryTransientMut::Weld(WeldWorldMut::MeteorFlash(
                entity
                    .into_mut::<NativeWeldMeteorFlashActorState>()
                    .expect("transient owns its component")
                    .into_inner(),
            )),
            TransientKind::WeldMeteorMarker => {
                PrimaryTransientMut::Weld(WeldWorldMut::MeteorMarker(
                    entity
                        .into_mut::<NativeWeldMeteorMarkerState>()
                        .expect("transient owns its component")
                        .into_inner(),
                ))
            }
            TransientKind::WeldPersistentEtherealBoulder => PrimaryTransientMut::Weld(
                WeldWorldMut::Persistent(WeldPersistentMut::EtherealBoulder(
                    entity
                        .into_mut::<NativeWeldEtherealBoulderState>()
                        .expect("transient owns its component")
                        .into_inner(),
                )),
            ),
            TransientKind::WeldPersistentHailstones => {
                PrimaryTransientMut::Weld(WeldWorldMut::Persistent(WeldPersistentMut::Hailstones(
                    entity
                        .into_mut::<NativeWeldHailstonesState>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::WeldPersistentMeteorField => {
                PrimaryTransientMut::Weld(WeldWorldMut::Persistent(WeldPersistentMut::MeteorField(
                    entity
                        .into_mut::<NativeWeldMeteorFieldState>()
                        .expect("transient owns its component")
                        .into_inner(),
                )))
            }
            TransientKind::WeldSteam => PrimaryTransientMut::Weld(WeldWorldMut::Steam(
                entity
                    .into_mut::<NativeWeldSteamActorState>()
                    .expect("transient owns its component")
                    .into_inner(),
            )),
            TransientKind::EarthBoulderBit => PrimaryTransientMut::EarthBoulderBit(
                entity
                    .into_mut::<PrimarySpellEarthBoulderBit>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::Air => PrimaryTransientMut::Air(
                entity
                    .into_mut::<PrimarySpellAirTransient>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::Water => PrimaryTransientMut::Water(
                entity
                    .into_mut::<PrimarySpellWaterTransient>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::EarthImpact => PrimaryTransientMut::EarthImpact(
                entity
                    .into_mut::<PrimarySpellEarthImpact>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::EarthCalledRock => PrimaryTransientMut::EarthCalledRock(
                entity
                    .into_mut::<PrimarySpellEarthCalledRock>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::EtherImpact => PrimaryTransientMut::EtherImpact(
                entity
                    .into_mut::<PrimarySpellEtherImpact>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::EtherPierceStreak => PrimaryTransientMut::EtherPierceStreak(
                entity
                    .into_mut::<PrimarySpellEtherPierceStreak>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::EtherBlast => PrimaryTransientMut::EtherBlast(
                entity
                    .into_mut::<PrimarySpellEtherBlast>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::FireParticle => PrimaryTransientMut::FireParticle(
                entity
                    .into_mut::<PrimarySpellFireParticle>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::FireImpact => PrimaryTransientMut::FireImpact(
                entity
                    .into_mut::<PrimarySpellFireImpact>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::FireEmber => PrimaryTransientMut::FireEmber(
                entity
                    .into_mut::<PrimarySpellFireEmber>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::FireExplosion => PrimaryTransientMut::FireExplosion(
                entity
                    .into_mut::<PrimarySpellFireExplosion>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::FireGoodImp => PrimaryTransientMut::FireGoodImp(
                entity
                    .into_mut::<PrimarySpellFireGoodImp>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
            TransientKind::FirePatch => PrimaryTransientMut::FirePatch(
                entity
                    .into_mut::<PrimarySpellFirePatch>()
                    .expect("transient owns its component")
                    .into_inner(),
            ),
        }
    }
    fn take(entity: &mut EntityWorldMut<'_>) -> PrimarySpellTransient {
        match *entity.get::<TransientKind>().expect("transient owns its kind") {
            TransientKind::AirHurricane => PrimarySpellTransient::AirHurricane(
                entity.take::<PrimarySpellAirHurricane>().expect("transient owns its component"),
            ),
            TransientKind::WaterAura => PrimarySpellTransient::WaterAura(
                entity.take::<PrimarySpellWaterAura>().expect("transient owns its component"),
            ),
            TransientKind::WaterHail => PrimarySpellTransient::WaterHail(
                entity.take::<PrimarySpellWaterHail>().expect("transient owns its component"),
            ),
            TransientKind::Harden => PrimarySpellTransient::Harden(
                entity.take::<NativeHardenEffect>().expect("transient owns its component"),
            ),
            TransientKind::StaffActionMelee => PrimarySpellTransient::Staff(
                NativePlayerStaffTransient::Action(NativePlayerStaffAction::Melee(
                    entity
                        .take::<NativePlayerStaffMeleeAction>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::StaffActionSpin => PrimarySpellTransient::Staff(
                NativePlayerStaffTransient::Action(NativePlayerStaffAction::Spin(
                    entity
                        .take::<NativePlayerStaffSpinAction>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::StaffContact => {
                PrimarySpellTransient::Staff(NativePlayerStaffTransient::Contact(
                    entity
                        .take::<NativePlayerStaffContactEvent>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::StaffContactKnockback => {
                PrimarySpellTransient::Staff(NativePlayerStaffTransient::ContactKnockback(
                    entity
                        .take::<NativePlayerStaffContactKnockback>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::StaffPikeBreak => {
                PrimarySpellTransient::Staff(NativePlayerStaffTransient::PikeBreak(
                    entity
                        .take::<NativePlayerStaffPikeBreakVfx>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::StaffVfxSmoke => PrimarySpellTransient::Staff(
                NativePlayerStaffTransient::Vfx(NativePlayerStaffVfx::Smoke(
                    entity
                        .take::<NativePlayerStaffSmokeVfx>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::StaffVfxMoveFade => PrimarySpellTransient::Staff(
                NativePlayerStaffTransient::Vfx(NativePlayerStaffVfx::MoveFade(
                    entity
                        .take::<NativePlayerStaffMoveFadeVfx>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::StaffVfxPerspectiveFade => PrimarySpellTransient::Staff(
                NativePlayerStaffTransient::Vfx(NativePlayerStaffVfx::PerspectiveFade(
                    entity
                        .take::<NativePlayerStaffPerspectiveVfx>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::StaffKnockback => {
                PrimarySpellTransient::Staff(NativePlayerStaffTransient::Knockback(
                    entity
                        .take::<NativeStaffKnockbackActor>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldBlizzardChainFrost => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::BlizzardChainFrost(
                    entity
                        .take::<NativeWeldBlizzardChainFrostState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldBlizzardGlow => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::BlizzardGlow(
                    entity
                        .take::<NativeWeldBlizzardGlowState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldBoulderDebris => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::BoulderDebris(
                    entity
                        .take::<NativeWeldBoulderDebrisActorState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldChannel => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::Channel(
                    entity
                        .take::<NativeWeldChannelActorState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldFlameLashFade => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::FlameLashFade(
                    entity
                        .take::<NativeWeldFlameLashFadeState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldFrostFade => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::FrostFade(
                    entity
                        .take::<NativeWeldFrostFadeActorState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldGroundSparkFade => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::GroundSparkFade(
                    entity
                        .take::<NativeWeldGroundSparkFadeActorState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldHailChildFlash => PrimarySpellTransient::Weld(
                NativeWeldWorldActor::HailChild(NativeWeldHailChildActorState::Flash(
                    entity
                        .take::<NativeWeldHailFlashState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldHailChildKnockback => PrimarySpellTransient::Weld(
                NativeWeldWorldActor::HailChild(NativeWeldHailChildActorState::Knockback(
                    entity
                        .take::<NativeWeldHailKnockbackState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldHailChildLine => PrimarySpellTransient::Weld(
                NativeWeldWorldActor::HailChild(NativeWeldHailChildActorState::Line(
                    entity.take::<NativeWeldHailLineState>().expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldHailChildTerrainBouncer => PrimarySpellTransient::Weld(
                NativeWeldWorldActor::HailChild(NativeWeldHailChildActorState::TerrainBouncer(
                    entity
                        .take::<NativeWeldHailTerrainBouncerState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldHailChildTerrainParticle => PrimarySpellTransient::Weld(
                NativeWeldWorldActor::HailChild(NativeWeldHailChildActorState::TerrainParticle(
                    entity
                        .take::<NativeWeldHailTerrainParticleState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldHailRockFade => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::HailRockFade(
                    entity
                        .take::<NativeWeldHailRockFadeActorState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldImpact => PrimarySpellTransient::Weld(NativeWeldWorldActor::Impact(
                entity.take::<NativeWeldImpactActorState>().expect("transient owns its component"),
            )),
            TransientKind::WeldMeteor => PrimarySpellTransient::Weld(NativeWeldWorldActor::Meteor(
                entity.take::<NativeWeldMeteorActorState>().expect("transient owns its component"),
            )),
            TransientKind::WeldMeteorFlash => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::MeteorFlash(
                    entity
                        .take::<NativeWeldMeteorFlashActorState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldMeteorMarker => {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::MeteorMarker(
                    entity
                        .take::<NativeWeldMeteorMarkerState>()
                        .expect("transient owns its component"),
                ))
            }
            TransientKind::WeldPersistentEtherealBoulder => PrimarySpellTransient::Weld(
                NativeWeldWorldActor::Persistent(NativeWeldPersistentActorState::EtherealBoulder(
                    entity
                        .take::<NativeWeldEtherealBoulderState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldPersistentHailstones => PrimarySpellTransient::Weld(
                NativeWeldWorldActor::Persistent(NativeWeldPersistentActorState::Hailstones(
                    entity
                        .take::<NativeWeldHailstonesState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldPersistentMeteorField => PrimarySpellTransient::Weld(
                NativeWeldWorldActor::Persistent(NativeWeldPersistentActorState::MeteorField(
                    entity
                        .take::<NativeWeldMeteorFieldState>()
                        .expect("transient owns its component"),
                )),
            ),
            TransientKind::WeldSteam => PrimarySpellTransient::Weld(NativeWeldWorldActor::Steam(
                entity.take::<NativeWeldSteamActorState>().expect("transient owns its component"),
            )),
            TransientKind::EarthBoulderBit => PrimarySpellTransient::EarthBoulderBit(
                entity.take::<PrimarySpellEarthBoulderBit>().expect("transient owns its component"),
            ),
            TransientKind::Air => PrimarySpellTransient::Air(
                entity.take::<PrimarySpellAirTransient>().expect("transient owns its component"),
            ),
            TransientKind::Water => PrimarySpellTransient::Water(
                entity.take::<PrimarySpellWaterTransient>().expect("transient owns its component"),
            ),
            TransientKind::EarthImpact => PrimarySpellTransient::EarthImpact(
                entity.take::<PrimarySpellEarthImpact>().expect("transient owns its component"),
            ),
            TransientKind::EarthCalledRock => PrimarySpellTransient::EarthCalledRock(
                entity.take::<PrimarySpellEarthCalledRock>().expect("transient owns its component"),
            ),
            TransientKind::EtherImpact => PrimarySpellTransient::EtherImpact(
                entity.take::<PrimarySpellEtherImpact>().expect("transient owns its component"),
            ),
            TransientKind::EtherPierceStreak => PrimarySpellTransient::EtherPierceStreak(
                entity
                    .take::<PrimarySpellEtherPierceStreak>()
                    .expect("transient owns its component"),
            ),
            TransientKind::EtherBlast => PrimarySpellTransient::EtherBlast(
                entity.take::<PrimarySpellEtherBlast>().expect("transient owns its component"),
            ),
            TransientKind::FireParticle => PrimarySpellTransient::FireParticle(
                entity.take::<PrimarySpellFireParticle>().expect("transient owns its component"),
            ),
            TransientKind::FireImpact => PrimarySpellTransient::FireImpact(
                entity.take::<PrimarySpellFireImpact>().expect("transient owns its component"),
            ),
            TransientKind::FireEmber => PrimarySpellTransient::FireEmber(
                entity.take::<PrimarySpellFireEmber>().expect("transient owns its component"),
            ),
            TransientKind::FireExplosion => PrimarySpellTransient::FireExplosion(
                entity.take::<PrimarySpellFireExplosion>().expect("transient owns its component"),
            ),
            TransientKind::FireGoodImp => PrimarySpellTransient::FireGoodImp(
                entity.take::<PrimarySpellFireGoodImp>().expect("transient owns its component"),
            ),
            TransientKind::FirePatch => PrimarySpellTransient::FirePatch(
                entity.take::<PrimarySpellFirePatch>().expect("transient owns its component"),
            ),
        }
    }
    fn spawn(world: &mut World, record: PrimarySpellTransient) -> Entity {
        match record {
            PrimarySpellTransient::AirHurricane(record) => {
                world.spawn((TransientKind::AirHurricane, record)).id()
            }
            PrimarySpellTransient::WaterAura(record) => {
                world.spawn((TransientKind::WaterAura, record)).id()
            }
            PrimarySpellTransient::WaterHail(record) => {
                world.spawn((TransientKind::WaterHail, record)).id()
            }
            PrimarySpellTransient::Harden(record) => {
                world.spawn((TransientKind::Harden, record)).id()
            }
            PrimarySpellTransient::Staff(NativePlayerStaffTransient::Action(
                NativePlayerStaffAction::Melee(record),
            )) => world.spawn((TransientKind::StaffActionMelee, record)).id(),
            PrimarySpellTransient::Staff(NativePlayerStaffTransient::Action(
                NativePlayerStaffAction::Spin(record),
            )) => world.spawn((TransientKind::StaffActionSpin, record)).id(),
            PrimarySpellTransient::Staff(NativePlayerStaffTransient::Contact(record)) => {
                world.spawn((TransientKind::StaffContact, record)).id()
            }
            PrimarySpellTransient::Staff(NativePlayerStaffTransient::ContactKnockback(record)) => {
                world.spawn((TransientKind::StaffContactKnockback, record)).id()
            }
            PrimarySpellTransient::Staff(NativePlayerStaffTransient::PikeBreak(record)) => {
                world.spawn((TransientKind::StaffPikeBreak, record)).id()
            }
            PrimarySpellTransient::Staff(NativePlayerStaffTransient::Vfx(
                NativePlayerStaffVfx::Smoke(record),
            )) => world.spawn((TransientKind::StaffVfxSmoke, record)).id(),
            PrimarySpellTransient::Staff(NativePlayerStaffTransient::Vfx(
                NativePlayerStaffVfx::MoveFade(record),
            )) => world.spawn((TransientKind::StaffVfxMoveFade, record)).id(),
            PrimarySpellTransient::Staff(NativePlayerStaffTransient::Vfx(
                NativePlayerStaffVfx::PerspectiveFade(record),
            )) => world.spawn((TransientKind::StaffVfxPerspectiveFade, record)).id(),
            PrimarySpellTransient::Staff(NativePlayerStaffTransient::Knockback(record)) => {
                world.spawn((TransientKind::StaffKnockback, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::BlizzardChainFrost(record)) => {
                world.spawn((TransientKind::WeldBlizzardChainFrost, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::BlizzardGlow(record)) => {
                world.spawn((TransientKind::WeldBlizzardGlow, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::BoulderDebris(record)) => {
                world.spawn((TransientKind::WeldBoulderDebris, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::Channel(record)) => {
                world.spawn((TransientKind::WeldChannel, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::FlameLashFade(record)) => {
                world.spawn((TransientKind::WeldFlameLashFade, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::FrostFade(record)) => {
                world.spawn((TransientKind::WeldFrostFade, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::GroundSparkFade(record)) => {
                world.spawn((TransientKind::WeldGroundSparkFade, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                NativeWeldHailChildActorState::Flash(record),
            )) => world.spawn((TransientKind::WeldHailChildFlash, record)).id(),
            PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                NativeWeldHailChildActorState::Knockback(record),
            )) => world.spawn((TransientKind::WeldHailChildKnockback, record)).id(),
            PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                NativeWeldHailChildActorState::Line(record),
            )) => world.spawn((TransientKind::WeldHailChildLine, record)).id(),
            PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                NativeWeldHailChildActorState::TerrainBouncer(record),
            )) => world.spawn((TransientKind::WeldHailChildTerrainBouncer, record)).id(),
            PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                NativeWeldHailChildActorState::TerrainParticle(record),
            )) => world.spawn((TransientKind::WeldHailChildTerrainParticle, record)).id(),
            PrimarySpellTransient::Weld(NativeWeldWorldActor::HailRockFade(record)) => {
                world.spawn((TransientKind::WeldHailRockFade, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::Impact(record)) => {
                world.spawn((TransientKind::WeldImpact, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::Meteor(record)) => {
                world.spawn((TransientKind::WeldMeteor, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::MeteorFlash(record)) => {
                world.spawn((TransientKind::WeldMeteorFlash, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::MeteorMarker(record)) => {
                world.spawn((TransientKind::WeldMeteorMarker, record)).id()
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(
                NativeWeldPersistentActorState::EtherealBoulder(record),
            )) => world.spawn((TransientKind::WeldPersistentEtherealBoulder, record)).id(),
            PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(
                NativeWeldPersistentActorState::Hailstones(record),
            )) => world.spawn((TransientKind::WeldPersistentHailstones, record)).id(),
            PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(
                NativeWeldPersistentActorState::MeteorField(record),
            )) => world.spawn((TransientKind::WeldPersistentMeteorField, record)).id(),
            PrimarySpellTransient::Weld(NativeWeldWorldActor::Steam(record)) => {
                world.spawn((TransientKind::WeldSteam, record)).id()
            }
            PrimarySpellTransient::EarthBoulderBit(record) => {
                world.spawn((TransientKind::EarthBoulderBit, record)).id()
            }
            PrimarySpellTransient::Air(record) => world.spawn((TransientKind::Air, record)).id(),
            PrimarySpellTransient::Water(record) => {
                world.spawn((TransientKind::Water, record)).id()
            }
            PrimarySpellTransient::EarthImpact(record) => {
                world.spawn((TransientKind::EarthImpact, record)).id()
            }
            PrimarySpellTransient::EarthCalledRock(record) => {
                world.spawn((TransientKind::EarthCalledRock, record)).id()
            }
            PrimarySpellTransient::EtherImpact(record) => {
                world.spawn((TransientKind::EtherImpact, record)).id()
            }
            PrimarySpellTransient::EtherPierceStreak(record) => {
                world.spawn((TransientKind::EtherPierceStreak, record)).id()
            }
            PrimarySpellTransient::EtherBlast(record) => {
                world.spawn((TransientKind::EtherBlast, record)).id()
            }
            PrimarySpellTransient::FireParticle(record) => {
                world.spawn((TransientKind::FireParticle, record)).id()
            }
            PrimarySpellTransient::FireImpact(record) => {
                world.spawn((TransientKind::FireImpact, record)).id()
            }
            PrimarySpellTransient::FireEmber(record) => {
                world.spawn((TransientKind::FireEmber, record)).id()
            }
            PrimarySpellTransient::FireExplosion(record) => {
                world.spawn((TransientKind::FireExplosion, record)).id()
            }
            PrimarySpellTransient::FireGoodImp(record) => {
                world.spawn((TransientKind::FireGoodImp, record)).id()
            }
            PrimarySpellTransient::FirePatch(record) => {
                world.spawn((TransientKind::FirePatch, record)).id()
            }
        }
    }
    pub fn push(&mut self, record: PrimarySpellTransient) {
        if let Some(births) = &mut self.births {
            births.push(record);
        } else {
            self.entities.spawn(|world| Self::spawn(world, record));
        }
    }
    pub fn append(&mut self, other: &mut Self) {
        self.extend(std::mem::take(other));
    }
    pub fn get(&self, index: usize) -> Option<PrimaryTransientRef<'_>> {
        self.entities.get(index).map(Self::read)
    }
    pub fn get_mut(&mut self, index: usize) -> Option<PrimaryTransientMut<'_>> {
        self.entities.get_mut(index).map(Self::write)
    }
    pub fn remove(&mut self, index: usize) -> PrimarySpellTransient {
        self.entities.remove(index, Self::take)
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&PrimaryTransientRef<'_>) -> bool) {
        self.entities.retain(|entity| keep(&Self::read(entity)));
    }
    pub fn retain_mut(&mut self, mut keep: impl FnMut(&mut PrimaryTransientMut<'_>) -> bool) {
        self.entities.retain_mut(|entity| keep(&mut Self::write(entity)));
    }
    pub fn clear(&mut self) {
        self.entities.clear();
        if let Some(births) = &mut self.births {
            births.clear();
        }
    }
    pub fn sort_by_key<K: Ord>(&mut self, mut key: impl FnMut(PrimaryTransientRef<'_>) -> K) {
        self.entities.sort_by_key(|entity| key(Self::read(entity)));
    }
    pub(crate) fn collect_births(&mut self) {
        assert!(self.is_empty() && self.births.is_none(), "transient births have one phase owner");
        self.births = Some(Vec::new());
    }
    pub(crate) fn take_births(&mut self) -> Vec<PrimarySpellTransient> {
        std::mem::take(self.births.as_mut().expect("transient birth collection is active"))
    }
    pub(crate) fn advance(
        &mut self,
        mut step: impl FnMut(&mut PrimaryTransientMut<'_>) -> EntityStep<PrimarySpellTransient>,
    ) {
        self.entities.advance_with_births(|entity| step(&mut Self::write(entity)), Self::spawn);
    }
}
impl Extend<PrimarySpellTransient> for PrimaryTransients {
    fn extend<I: IntoIterator<Item = PrimarySpellTransient>>(&mut self, records: I) {
        for record in records {
            self.push(record);
        }
    }
}
impl FromIterator<PrimarySpellTransient> for PrimaryTransients {
    fn from_iter<I: IntoIterator<Item = PrimarySpellTransient>>(records: I) -> Self {
        let mut result = Self::default();
        result.extend(records);
        result
    }
}
impl IntoIterator for PrimaryTransients {
    type Item = PrimarySpellTransient;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.entities.into_records(Self::take).into_iter()
    }
}
impl Clone for PrimaryTransients {
    fn clone(&self) -> Self {
        let mut result: Self = self.iter().map(|row| row.snapshot()).collect();
        result.births.clone_from(&self.births);
        result
    }
}
impl fmt::Debug for PrimaryTransients {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl PartialEq for PrimaryTransients {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter()) && self.births == other.births
    }
}
#[cfg(feature = "persistence")]
impl serde::Serialize for PrimaryTransients {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}
#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for PrimaryTransients {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<PrimarySpellTransient>::deserialize(deserializer)
            .map(|records| records.into_iter().collect())
    }
}
