//! Borrowed views over concrete transient components. The owned enums remain
//! constructor and save records; viewing an effect never clones its program.

use crate::harden_effects::NativeHardenEffect;
use crate::spells::model::*;
use crate::spells::staff::*;
use crate::spells::weld::*;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Serialize),
    serde(rename = "PrimarySpellTransient")
)]
pub enum PrimaryTransientRef<'a> {
    AirHurricane(&'a PrimarySpellAirHurricane),
    WaterAura(&'a PrimarySpellWaterAura),
    WaterHail(&'a PrimarySpellWaterHail),
    Harden(&'a NativeHardenEffect),
    Staff(StaffTransientRef<'a>),
    Weld(WeldWorldRef<'a>),
    EarthBoulderBit(&'a PrimarySpellEarthBoulderBit),
    Air(&'a PrimarySpellAirTransient),
    Water(&'a PrimarySpellWaterTransient),
    EarthImpact(&'a PrimarySpellEarthImpact),
    EarthCalledRock(&'a PrimarySpellEarthCalledRock),
    EtherImpact(&'a PrimarySpellEtherImpact),
    EtherPierceStreak(&'a PrimarySpellEtherPierceStreak),
    EtherBlast(&'a PrimarySpellEtherBlast),
    FireParticle(&'a PrimarySpellFireParticle),
    FireImpact(&'a PrimarySpellFireImpact),
    FireEmber(&'a PrimarySpellFireEmber),
    FireExplosion(&'a PrimarySpellFireExplosion),
    FireGoodImp(&'a PrimarySpellFireGoodImp),
    FirePatch(&'a PrimarySpellFirePatch),
}

pub enum PrimaryTransientMut<'a> {
    AirHurricane(&'a mut PrimarySpellAirHurricane),
    WaterAura(&'a mut PrimarySpellWaterAura),
    WaterHail(&'a mut PrimarySpellWaterHail),
    Harden(&'a mut NativeHardenEffect),
    Staff(StaffTransientMut<'a>),
    Weld(WeldWorldMut<'a>),
    EarthBoulderBit(&'a mut PrimarySpellEarthBoulderBit),
    Air(&'a mut PrimarySpellAirTransient),
    Water(&'a mut PrimarySpellWaterTransient),
    EarthImpact(&'a mut PrimarySpellEarthImpact),
    EarthCalledRock(&'a mut PrimarySpellEarthCalledRock),
    EtherImpact(&'a mut PrimarySpellEtherImpact),
    EtherPierceStreak(&'a mut PrimarySpellEtherPierceStreak),
    EtherBlast(&'a mut PrimarySpellEtherBlast),
    FireParticle(&'a mut PrimarySpellFireParticle),
    FireImpact(&'a mut PrimarySpellFireImpact),
    FireEmber(&'a mut PrimarySpellFireEmber),
    FireExplosion(&'a mut PrimarySpellFireExplosion),
    FireGoodImp(&'a mut PrimarySpellFireGoodImp),
    FirePatch(&'a mut PrimarySpellFirePatch),
}
impl PrimarySpellTransient {
    pub fn view(&self) -> PrimaryTransientRef<'_> {
        match self {
            Self::AirHurricane(row) => PrimaryTransientRef::AirHurricane(row),
            Self::WaterAura(row) => PrimaryTransientRef::WaterAura(row),
            Self::WaterHail(row) => PrimaryTransientRef::WaterHail(row),
            Self::Harden(row) => PrimaryTransientRef::Harden(row),
            Self::Staff(row) => PrimaryTransientRef::Staff(row.view()),
            Self::Weld(row) => PrimaryTransientRef::Weld(row.view()),
            Self::EarthBoulderBit(row) => PrimaryTransientRef::EarthBoulderBit(row),
            Self::Air(row) => PrimaryTransientRef::Air(row),
            Self::Water(row) => PrimaryTransientRef::Water(row),
            Self::EarthImpact(row) => PrimaryTransientRef::EarthImpact(row),
            Self::EarthCalledRock(row) => PrimaryTransientRef::EarthCalledRock(row),
            Self::EtherImpact(row) => PrimaryTransientRef::EtherImpact(row),
            Self::EtherPierceStreak(row) => PrimaryTransientRef::EtherPierceStreak(row),
            Self::EtherBlast(row) => PrimaryTransientRef::EtherBlast(row),
            Self::FireParticle(row) => PrimaryTransientRef::FireParticle(row),
            Self::FireImpact(row) => PrimaryTransientRef::FireImpact(row),
            Self::FireEmber(row) => PrimaryTransientRef::FireEmber(row),
            Self::FireExplosion(row) => PrimaryTransientRef::FireExplosion(row),
            Self::FireGoodImp(row) => PrimaryTransientRef::FireGoodImp(row),
            Self::FirePatch(row) => PrimaryTransientRef::FirePatch(row),
        }
    }
    pub fn view_mut(&mut self) -> PrimaryTransientMut<'_> {
        match self {
            Self::AirHurricane(row) => PrimaryTransientMut::AirHurricane(row),
            Self::WaterAura(row) => PrimaryTransientMut::WaterAura(row),
            Self::WaterHail(row) => PrimaryTransientMut::WaterHail(row),
            Self::Harden(row) => PrimaryTransientMut::Harden(row),
            Self::Staff(row) => PrimaryTransientMut::Staff(row.view_mut()),
            Self::Weld(row) => PrimaryTransientMut::Weld(row.view_mut()),
            Self::EarthBoulderBit(row) => PrimaryTransientMut::EarthBoulderBit(row),
            Self::Air(row) => PrimaryTransientMut::Air(row),
            Self::Water(row) => PrimaryTransientMut::Water(row),
            Self::EarthImpact(row) => PrimaryTransientMut::EarthImpact(row),
            Self::EarthCalledRock(row) => PrimaryTransientMut::EarthCalledRock(row),
            Self::EtherImpact(row) => PrimaryTransientMut::EtherImpact(row),
            Self::EtherPierceStreak(row) => PrimaryTransientMut::EtherPierceStreak(row),
            Self::EtherBlast(row) => PrimaryTransientMut::EtherBlast(row),
            Self::FireParticle(row) => PrimaryTransientMut::FireParticle(row),
            Self::FireImpact(row) => PrimaryTransientMut::FireImpact(row),
            Self::FireEmber(row) => PrimaryTransientMut::FireEmber(row),
            Self::FireExplosion(row) => PrimaryTransientMut::FireExplosion(row),
            Self::FireGoodImp(row) => PrimaryTransientMut::FireGoodImp(row),
            Self::FirePatch(row) => PrimaryTransientMut::FirePatch(row),
        }
    }
}
impl<'a> PrimaryTransientRef<'a> {
    pub fn common(self) -> &'a PrimarySpellTransientBase {
        match self {
            Self::AirHurricane(row) => &row.common,
            Self::WaterAura(row) => &row.common,
            Self::WaterHail(row) => &row.common,
            Self::Harden(row) => row.common(),
            Self::Staff(row) => row.common(),
            Self::Weld(row) => &row.common().owner,
            Self::EarthBoulderBit(row) => &row.common,
            Self::Air(row) => &row.common,
            Self::Water(row) => &row.common,
            Self::EarthImpact(row) => &row.common,
            Self::EarthCalledRock(row) => &row.common,
            Self::EtherImpact(row) => &row.common,
            Self::EtherPierceStreak(row) => &row.common,
            Self::EtherBlast(row) => &row.common,
            Self::FireParticle(row) => &row.common,
            Self::FireImpact(row) => &row.common,
            Self::FireEmber(row) => &row.common,
            Self::FireExplosion(row) => &row.common,
            Self::FireGoodImp(row) => &row.common,
            Self::FirePatch(row) => &row.common,
        }
    }
    pub fn snapshot(self) -> PrimarySpellTransient {
        match self {
            Self::AirHurricane(row) => PrimarySpellTransient::AirHurricane(row.clone()),
            Self::WaterAura(row) => PrimarySpellTransient::WaterAura(row.clone()),
            Self::WaterHail(row) => PrimarySpellTransient::WaterHail(row.clone()),
            Self::Harden(row) => PrimarySpellTransient::Harden(row.clone()),
            Self::Staff(row) => PrimarySpellTransient::Staff(row.snapshot()),
            Self::Weld(row) => PrimarySpellTransient::Weld(row.snapshot()),
            Self::EarthBoulderBit(row) => PrimarySpellTransient::EarthBoulderBit(row.clone()),
            Self::Air(row) => PrimarySpellTransient::Air(row.clone()),
            Self::Water(row) => PrimarySpellTransient::Water(row.clone()),
            Self::EarthImpact(row) => PrimarySpellTransient::EarthImpact(row.clone()),
            Self::EarthCalledRock(row) => PrimarySpellTransient::EarthCalledRock(row.clone()),
            Self::EtherImpact(row) => PrimarySpellTransient::EtherImpact(row.clone()),
            Self::EtherPierceStreak(row) => PrimarySpellTransient::EtherPierceStreak(row.clone()),
            Self::EtherBlast(row) => PrimarySpellTransient::EtherBlast(row.clone()),
            Self::FireParticle(row) => PrimarySpellTransient::FireParticle(row.clone()),
            Self::FireImpact(row) => PrimarySpellTransient::FireImpact(row.clone()),
            Self::FireEmber(row) => PrimarySpellTransient::FireEmber(row.clone()),
            Self::FireExplosion(row) => PrimarySpellTransient::FireExplosion(row.clone()),
            Self::FireGoodImp(row) => PrimarySpellTransient::FireGoodImp(row.clone()),
            Self::FirePatch(row) => PrimarySpellTransient::FirePatch(row.clone()),
        }
    }
}
impl PrimaryTransientMut<'_> {
    pub fn view(&self) -> PrimaryTransientRef<'_> {
        match self {
            Self::AirHurricane(row) => PrimaryTransientRef::AirHurricane(row),
            Self::WaterAura(row) => PrimaryTransientRef::WaterAura(row),
            Self::WaterHail(row) => PrimaryTransientRef::WaterHail(row),
            Self::Harden(row) => PrimaryTransientRef::Harden(row),
            Self::Staff(row) => PrimaryTransientRef::Staff(row.view()),
            Self::Weld(row) => PrimaryTransientRef::Weld(row.view()),
            Self::EarthBoulderBit(row) => PrimaryTransientRef::EarthBoulderBit(row),
            Self::Air(row) => PrimaryTransientRef::Air(row),
            Self::Water(row) => PrimaryTransientRef::Water(row),
            Self::EarthImpact(row) => PrimaryTransientRef::EarthImpact(row),
            Self::EarthCalledRock(row) => PrimaryTransientRef::EarthCalledRock(row),
            Self::EtherImpact(row) => PrimaryTransientRef::EtherImpact(row),
            Self::EtherPierceStreak(row) => PrimaryTransientRef::EtherPierceStreak(row),
            Self::EtherBlast(row) => PrimaryTransientRef::EtherBlast(row),
            Self::FireParticle(row) => PrimaryTransientRef::FireParticle(row),
            Self::FireImpact(row) => PrimaryTransientRef::FireImpact(row),
            Self::FireEmber(row) => PrimaryTransientRef::FireEmber(row),
            Self::FireExplosion(row) => PrimaryTransientRef::FireExplosion(row),
            Self::FireGoodImp(row) => PrimaryTransientRef::FireGoodImp(row),
            Self::FirePatch(row) => PrimaryTransientRef::FirePatch(row),
        }
    }
    pub fn common(&self) -> &PrimarySpellTransientBase {
        self.view().common()
    }
    pub fn common_mut(&mut self) -> &mut PrimarySpellTransientBase {
        match self {
            Self::AirHurricane(row) => &mut row.common,
            Self::WaterAura(row) => &mut row.common,
            Self::WaterHail(row) => &mut row.common,
            Self::Harden(row) => row.common_mut(),
            Self::Staff(row) => row.common_mut(),
            Self::Weld(row) => &mut row.common_mut().owner,
            Self::EarthBoulderBit(row) => &mut row.common,
            Self::Air(row) => &mut row.common,
            Self::Water(row) => &mut row.common,
            Self::EarthImpact(row) => &mut row.common,
            Self::EarthCalledRock(row) => &mut row.common,
            Self::EtherImpact(row) => &mut row.common,
            Self::EtherPierceStreak(row) => &mut row.common,
            Self::EtherBlast(row) => &mut row.common,
            Self::FireParticle(row) => &mut row.common,
            Self::FireImpact(row) => &mut row.common,
            Self::FireEmber(row) => &mut row.common,
            Self::FireExplosion(row) => &mut row.common,
            Self::FireGoodImp(row) => &mut row.common,
            Self::FirePatch(row) => &mut row.common,
        }
    }
    pub fn replace(&mut self, record: PrimarySpellTransient) {
        match (self, record) {
            (Self::AirHurricane(stored), PrimarySpellTransient::AirHurricane(record)) => {
                **stored = record
            }
            (Self::WaterAura(stored), PrimarySpellTransient::WaterAura(record)) => {
                **stored = record
            }
            (Self::WaterHail(stored), PrimarySpellTransient::WaterHail(record)) => {
                **stored = record
            }
            (Self::Harden(stored), PrimarySpellTransient::Harden(record)) => **stored = record,
            (Self::Staff(stored), PrimarySpellTransient::Staff(record)) => stored.replace(record),
            (Self::Weld(stored), PrimarySpellTransient::Weld(record)) => stored.replace(record),
            (Self::EarthBoulderBit(stored), PrimarySpellTransient::EarthBoulderBit(record)) => {
                **stored = record
            }
            (Self::Air(stored), PrimarySpellTransient::Air(record)) => **stored = record,
            (Self::Water(stored), PrimarySpellTransient::Water(record)) => **stored = record,
            (Self::EarthImpact(stored), PrimarySpellTransient::EarthImpact(record)) => {
                **stored = record
            }
            (Self::EarthCalledRock(stored), PrimarySpellTransient::EarthCalledRock(record)) => {
                **stored = record
            }
            (Self::EtherImpact(stored), PrimarySpellTransient::EtherImpact(record)) => {
                **stored = record
            }
            (Self::EtherPierceStreak(stored), PrimarySpellTransient::EtherPierceStreak(record)) => {
                **stored = record
            }
            (Self::EtherBlast(stored), PrimarySpellTransient::EtherBlast(record)) => {
                **stored = record
            }
            (Self::FireParticle(stored), PrimarySpellTransient::FireParticle(record)) => {
                **stored = record
            }
            (Self::FireImpact(stored), PrimarySpellTransient::FireImpact(record)) => {
                **stored = record
            }
            (Self::FireEmber(stored), PrimarySpellTransient::FireEmber(record)) => {
                **stored = record
            }
            (Self::FireExplosion(stored), PrimarySpellTransient::FireExplosion(record)) => {
                **stored = record
            }
            (Self::FireGoodImp(stored), PrimarySpellTransient::FireGoodImp(record)) => {
                **stored = record
            }
            (Self::FirePatch(stored), PrimarySpellTransient::FirePatch(record)) => {
                **stored = record
            }
            _ => panic!("retained effect keeps its component type"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Serialize),
    serde(rename = "NativePlayerStaffTransient")
)]
pub enum StaffTransientRef<'a> {
    Action(StaffActionRef<'a>),
    Contact(&'a NativePlayerStaffContactEvent),
    ContactKnockback(&'a NativePlayerStaffContactKnockback),
    PikeBreak(&'a NativePlayerStaffPikeBreakVfx),
    Vfx(StaffVfxRef<'a>),
    Knockback(&'a NativeStaffKnockbackActor),
}

pub enum StaffTransientMut<'a> {
    Action(StaffActionMut<'a>),
    Contact(&'a mut NativePlayerStaffContactEvent),
    ContactKnockback(&'a mut NativePlayerStaffContactKnockback),
    PikeBreak(&'a mut NativePlayerStaffPikeBreakVfx),
    Vfx(StaffVfxMut<'a>),
    Knockback(&'a mut NativeStaffKnockbackActor),
}
impl NativePlayerStaffTransient {
    pub fn view(&self) -> StaffTransientRef<'_> {
        match self {
            Self::Action(row) => StaffTransientRef::Action(row.view()),
            Self::Contact(row) => StaffTransientRef::Contact(row),
            Self::ContactKnockback(row) => StaffTransientRef::ContactKnockback(row),
            Self::PikeBreak(row) => StaffTransientRef::PikeBreak(row),
            Self::Vfx(row) => StaffTransientRef::Vfx(row.view()),
            Self::Knockback(row) => StaffTransientRef::Knockback(row),
        }
    }
    pub fn view_mut(&mut self) -> StaffTransientMut<'_> {
        match self {
            Self::Action(row) => StaffTransientMut::Action(row.view_mut()),
            Self::Contact(row) => StaffTransientMut::Contact(row),
            Self::ContactKnockback(row) => StaffTransientMut::ContactKnockback(row),
            Self::PikeBreak(row) => StaffTransientMut::PikeBreak(row),
            Self::Vfx(row) => StaffTransientMut::Vfx(row.view_mut()),
            Self::Knockback(row) => StaffTransientMut::Knockback(row),
        }
    }
}
impl<'a> StaffTransientRef<'a> {
    pub fn common(self) -> &'a PrimarySpellTransientBase {
        match self {
            Self::Action(row) => row.common(),
            Self::Contact(row) => &row.common,
            Self::ContactKnockback(row) => &row.common,
            Self::PikeBreak(row) => &row.common,
            Self::Vfx(row) => row.common(),
            Self::Knockback(row) => &row.common,
        }
    }
    pub fn snapshot(self) -> NativePlayerStaffTransient {
        match self {
            Self::Action(row) => NativePlayerStaffTransient::Action(row.snapshot()),
            Self::Contact(row) => NativePlayerStaffTransient::Contact(row.clone()),
            Self::ContactKnockback(row) => {
                NativePlayerStaffTransient::ContactKnockback(row.clone())
            }
            Self::PikeBreak(row) => NativePlayerStaffTransient::PikeBreak(row.clone()),
            Self::Vfx(row) => NativePlayerStaffTransient::Vfx(row.snapshot()),
            Self::Knockback(row) => NativePlayerStaffTransient::Knockback(row.clone()),
        }
    }
}
impl StaffTransientMut<'_> {
    pub fn view(&self) -> StaffTransientRef<'_> {
        match self {
            Self::Action(row) => StaffTransientRef::Action(row.view()),
            Self::Contact(row) => StaffTransientRef::Contact(row),
            Self::ContactKnockback(row) => StaffTransientRef::ContactKnockback(row),
            Self::PikeBreak(row) => StaffTransientRef::PikeBreak(row),
            Self::Vfx(row) => StaffTransientRef::Vfx(row.view()),
            Self::Knockback(row) => StaffTransientRef::Knockback(row),
        }
    }
    pub fn common(&self) -> &PrimarySpellTransientBase {
        self.view().common()
    }
    pub fn common_mut(&mut self) -> &mut PrimarySpellTransientBase {
        match self {
            Self::Action(row) => row.common_mut(),
            Self::Contact(row) => &mut row.common,
            Self::ContactKnockback(row) => &mut row.common,
            Self::PikeBreak(row) => &mut row.common,
            Self::Vfx(row) => row.common_mut(),
            Self::Knockback(row) => &mut row.common,
        }
    }
    pub fn replace(&mut self, record: NativePlayerStaffTransient) {
        match (self, record) {
            (Self::Action(stored), NativePlayerStaffTransient::Action(record)) => {
                stored.replace(record)
            }
            (Self::Contact(stored), NativePlayerStaffTransient::Contact(record)) => {
                **stored = record
            }
            (
                Self::ContactKnockback(stored),
                NativePlayerStaffTransient::ContactKnockback(record),
            ) => **stored = record,
            (Self::PikeBreak(stored), NativePlayerStaffTransient::PikeBreak(record)) => {
                **stored = record
            }
            (Self::Vfx(stored), NativePlayerStaffTransient::Vfx(record)) => stored.replace(record),
            (Self::Knockback(stored), NativePlayerStaffTransient::Knockback(record)) => {
                **stored = record
            }
            _ => panic!("retained effect keeps its component type"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Serialize),
    serde(rename = "NativePlayerStaffAction")
)]
pub enum StaffActionRef<'a> {
    Melee(&'a NativePlayerStaffMeleeAction),
    Spin(&'a NativePlayerStaffSpinAction),
}

pub enum StaffActionMut<'a> {
    Melee(&'a mut NativePlayerStaffMeleeAction),
    Spin(&'a mut NativePlayerStaffSpinAction),
}
impl NativePlayerStaffAction {
    pub fn view(&self) -> StaffActionRef<'_> {
        match self {
            Self::Melee(row) => StaffActionRef::Melee(row),
            Self::Spin(row) => StaffActionRef::Spin(row),
        }
    }
    pub fn view_mut(&mut self) -> StaffActionMut<'_> {
        match self {
            Self::Melee(row) => StaffActionMut::Melee(row),
            Self::Spin(row) => StaffActionMut::Spin(row),
        }
    }
}
impl<'a> StaffActionRef<'a> {
    pub fn common(self) -> &'a PrimarySpellTransientBase {
        match self {
            Self::Melee(row) => &row.base.common,
            Self::Spin(row) => &row.base.common,
        }
    }
    pub fn snapshot(self) -> NativePlayerStaffAction {
        match self {
            Self::Melee(row) => NativePlayerStaffAction::Melee(row.clone()),
            Self::Spin(row) => NativePlayerStaffAction::Spin(row.clone()),
        }
    }
}
impl StaffActionMut<'_> {
    pub fn view(&self) -> StaffActionRef<'_> {
        match self {
            Self::Melee(row) => StaffActionRef::Melee(row),
            Self::Spin(row) => StaffActionRef::Spin(row),
        }
    }
    pub fn common(&self) -> &PrimarySpellTransientBase {
        self.view().common()
    }
    pub fn common_mut(&mut self) -> &mut PrimarySpellTransientBase {
        match self {
            Self::Melee(row) => &mut row.base.common,
            Self::Spin(row) => &mut row.base.common,
        }
    }
    pub fn replace(&mut self, record: NativePlayerStaffAction) {
        match (self, record) {
            (Self::Melee(stored), NativePlayerStaffAction::Melee(record)) => **stored = record,
            (Self::Spin(stored), NativePlayerStaffAction::Spin(record)) => **stored = record,
            _ => panic!("retained effect keeps its component type"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Serialize),
    serde(rename = "NativePlayerStaffVfx")
)]
pub enum StaffVfxRef<'a> {
    Smoke(&'a NativePlayerStaffSmokeVfx),
    MoveFade(&'a NativePlayerStaffMoveFadeVfx),
    PerspectiveFade(&'a NativePlayerStaffPerspectiveVfx),
}

pub enum StaffVfxMut<'a> {
    Smoke(&'a mut NativePlayerStaffSmokeVfx),
    MoveFade(&'a mut NativePlayerStaffMoveFadeVfx),
    PerspectiveFade(&'a mut NativePlayerStaffPerspectiveVfx),
}
impl NativePlayerStaffVfx {
    pub fn view(&self) -> StaffVfxRef<'_> {
        match self {
            Self::Smoke(row) => StaffVfxRef::Smoke(row),
            Self::MoveFade(row) => StaffVfxRef::MoveFade(row),
            Self::PerspectiveFade(row) => StaffVfxRef::PerspectiveFade(row),
        }
    }
    pub fn view_mut(&mut self) -> StaffVfxMut<'_> {
        match self {
            Self::Smoke(row) => StaffVfxMut::Smoke(row),
            Self::MoveFade(row) => StaffVfxMut::MoveFade(row),
            Self::PerspectiveFade(row) => StaffVfxMut::PerspectiveFade(row),
        }
    }
}
impl<'a> StaffVfxRef<'a> {
    pub fn common(self) -> &'a PrimarySpellTransientBase {
        match self {
            Self::Smoke(row) => &row.common,
            Self::MoveFade(row) => &row.common,
            Self::PerspectiveFade(row) => &row.common,
        }
    }
    pub fn snapshot(self) -> NativePlayerStaffVfx {
        match self {
            Self::Smoke(row) => NativePlayerStaffVfx::Smoke(row.clone()),
            Self::MoveFade(row) => NativePlayerStaffVfx::MoveFade(row.clone()),
            Self::PerspectiveFade(row) => NativePlayerStaffVfx::PerspectiveFade(row.clone()),
        }
    }
}
impl StaffVfxMut<'_> {
    pub fn view(&self) -> StaffVfxRef<'_> {
        match self {
            Self::Smoke(row) => StaffVfxRef::Smoke(row),
            Self::MoveFade(row) => StaffVfxRef::MoveFade(row),
            Self::PerspectiveFade(row) => StaffVfxRef::PerspectiveFade(row),
        }
    }
    pub fn common(&self) -> &PrimarySpellTransientBase {
        self.view().common()
    }
    pub fn common_mut(&mut self) -> &mut PrimarySpellTransientBase {
        match self {
            Self::Smoke(row) => &mut row.common,
            Self::MoveFade(row) => &mut row.common,
            Self::PerspectiveFade(row) => &mut row.common,
        }
    }
    pub fn replace(&mut self, record: NativePlayerStaffVfx) {
        match (self, record) {
            (Self::Smoke(stored), NativePlayerStaffVfx::Smoke(record)) => **stored = record,
            (Self::MoveFade(stored), NativePlayerStaffVfx::MoveFade(record)) => **stored = record,
            (Self::PerspectiveFade(stored), NativePlayerStaffVfx::PerspectiveFade(record)) => {
                **stored = record
            }
            _ => panic!("retained effect keeps its component type"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Serialize),
    serde(rename = "NativeWeldWorldActor")
)]
pub enum WeldWorldRef<'a> {
    BlizzardChainFrost(&'a NativeWeldBlizzardChainFrostState),
    BlizzardGlow(&'a NativeWeldBlizzardGlowState),
    BoulderDebris(&'a NativeWeldBoulderDebrisActorState),
    Channel(&'a NativeWeldChannelActorState),
    FlameLashFade(&'a NativeWeldFlameLashFadeState),
    FrostFade(&'a NativeWeldFrostFadeActorState),
    GroundSparkFade(&'a NativeWeldGroundSparkFadeActorState),
    HailChild(WeldHailChildRef<'a>),
    HailRockFade(&'a NativeWeldHailRockFadeActorState),
    Impact(&'a NativeWeldImpactActorState),
    Meteor(&'a NativeWeldMeteorActorState),
    MeteorFlash(&'a NativeWeldMeteorFlashActorState),
    MeteorMarker(&'a NativeWeldMeteorMarkerState),
    Persistent(WeldPersistentRef<'a>),
    Steam(&'a NativeWeldSteamActorState),
}

pub enum WeldWorldMut<'a> {
    BlizzardChainFrost(&'a mut NativeWeldBlizzardChainFrostState),
    BlizzardGlow(&'a mut NativeWeldBlizzardGlowState),
    BoulderDebris(&'a mut NativeWeldBoulderDebrisActorState),
    Channel(&'a mut NativeWeldChannelActorState),
    FlameLashFade(&'a mut NativeWeldFlameLashFadeState),
    FrostFade(&'a mut NativeWeldFrostFadeActorState),
    GroundSparkFade(&'a mut NativeWeldGroundSparkFadeActorState),
    HailChild(WeldHailChildMut<'a>),
    HailRockFade(&'a mut NativeWeldHailRockFadeActorState),
    Impact(&'a mut NativeWeldImpactActorState),
    Meteor(&'a mut NativeWeldMeteorActorState),
    MeteorFlash(&'a mut NativeWeldMeteorFlashActorState),
    MeteorMarker(&'a mut NativeWeldMeteorMarkerState),
    Persistent(WeldPersistentMut<'a>),
    Steam(&'a mut NativeWeldSteamActorState),
}
impl NativeWeldWorldActor {
    pub fn view(&self) -> WeldWorldRef<'_> {
        match self {
            Self::BlizzardChainFrost(row) => WeldWorldRef::BlizzardChainFrost(row),
            Self::BlizzardGlow(row) => WeldWorldRef::BlizzardGlow(row),
            Self::BoulderDebris(row) => WeldWorldRef::BoulderDebris(row),
            Self::Channel(row) => WeldWorldRef::Channel(row),
            Self::FlameLashFade(row) => WeldWorldRef::FlameLashFade(row),
            Self::FrostFade(row) => WeldWorldRef::FrostFade(row),
            Self::GroundSparkFade(row) => WeldWorldRef::GroundSparkFade(row),
            Self::HailChild(row) => WeldWorldRef::HailChild(row.view()),
            Self::HailRockFade(row) => WeldWorldRef::HailRockFade(row),
            Self::Impact(row) => WeldWorldRef::Impact(row),
            Self::Meteor(row) => WeldWorldRef::Meteor(row),
            Self::MeteorFlash(row) => WeldWorldRef::MeteorFlash(row),
            Self::MeteorMarker(row) => WeldWorldRef::MeteorMarker(row),
            Self::Persistent(row) => WeldWorldRef::Persistent(row.view()),
            Self::Steam(row) => WeldWorldRef::Steam(row),
        }
    }
    pub fn view_mut(&mut self) -> WeldWorldMut<'_> {
        match self {
            Self::BlizzardChainFrost(row) => WeldWorldMut::BlizzardChainFrost(row),
            Self::BlizzardGlow(row) => WeldWorldMut::BlizzardGlow(row),
            Self::BoulderDebris(row) => WeldWorldMut::BoulderDebris(row),
            Self::Channel(row) => WeldWorldMut::Channel(row),
            Self::FlameLashFade(row) => WeldWorldMut::FlameLashFade(row),
            Self::FrostFade(row) => WeldWorldMut::FrostFade(row),
            Self::GroundSparkFade(row) => WeldWorldMut::GroundSparkFade(row),
            Self::HailChild(row) => WeldWorldMut::HailChild(row.view_mut()),
            Self::HailRockFade(row) => WeldWorldMut::HailRockFade(row),
            Self::Impact(row) => WeldWorldMut::Impact(row),
            Self::Meteor(row) => WeldWorldMut::Meteor(row),
            Self::MeteorFlash(row) => WeldWorldMut::MeteorFlash(row),
            Self::MeteorMarker(row) => WeldWorldMut::MeteorMarker(row),
            Self::Persistent(row) => WeldWorldMut::Persistent(row.view_mut()),
            Self::Steam(row) => WeldWorldMut::Steam(row),
        }
    }
}
impl<'a> WeldWorldRef<'a> {
    pub fn common(self) -> &'a NativeWeldActorBase {
        match self {
            Self::BlizzardChainFrost(row) => &row.common,
            Self::BlizzardGlow(row) => &row.common,
            Self::BoulderDebris(row) => &row.common,
            Self::Channel(row) => &row.common,
            Self::FlameLashFade(row) => &row.common,
            Self::FrostFade(row) => &row.common,
            Self::GroundSparkFade(row) => &row.common,
            Self::HailChild(row) => row.common(),
            Self::HailRockFade(row) => &row.common,
            Self::Impact(row) => &row.common,
            Self::Meteor(row) => &row.common,
            Self::MeteorFlash(row) => &row.common,
            Self::MeteorMarker(row) => &row.common,
            Self::Persistent(row) => row.common(),
            Self::Steam(row) => &row.common,
        }
    }
    pub fn snapshot(self) -> NativeWeldWorldActor {
        match self {
            Self::BlizzardChainFrost(row) => NativeWeldWorldActor::BlizzardChainFrost(row.clone()),
            Self::BlizzardGlow(row) => NativeWeldWorldActor::BlizzardGlow(row.clone()),
            Self::BoulderDebris(row) => NativeWeldWorldActor::BoulderDebris(row.clone()),
            Self::Channel(row) => NativeWeldWorldActor::Channel(row.clone()),
            Self::FlameLashFade(row) => NativeWeldWorldActor::FlameLashFade(row.clone()),
            Self::FrostFade(row) => NativeWeldWorldActor::FrostFade(row.clone()),
            Self::GroundSparkFade(row) => NativeWeldWorldActor::GroundSparkFade(row.clone()),
            Self::HailChild(row) => NativeWeldWorldActor::HailChild(row.snapshot()),
            Self::HailRockFade(row) => NativeWeldWorldActor::HailRockFade(row.clone()),
            Self::Impact(row) => NativeWeldWorldActor::Impact(row.clone()),
            Self::Meteor(row) => NativeWeldWorldActor::Meteor(row.clone()),
            Self::MeteorFlash(row) => NativeWeldWorldActor::MeteorFlash(row.clone()),
            Self::MeteorMarker(row) => NativeWeldWorldActor::MeteorMarker(row.clone()),
            Self::Persistent(row) => NativeWeldWorldActor::Persistent(row.snapshot()),
            Self::Steam(row) => NativeWeldWorldActor::Steam(row.clone()),
        }
    }
}
impl WeldWorldMut<'_> {
    pub fn view(&self) -> WeldWorldRef<'_> {
        match self {
            Self::BlizzardChainFrost(row) => WeldWorldRef::BlizzardChainFrost(row),
            Self::BlizzardGlow(row) => WeldWorldRef::BlizzardGlow(row),
            Self::BoulderDebris(row) => WeldWorldRef::BoulderDebris(row),
            Self::Channel(row) => WeldWorldRef::Channel(row),
            Self::FlameLashFade(row) => WeldWorldRef::FlameLashFade(row),
            Self::FrostFade(row) => WeldWorldRef::FrostFade(row),
            Self::GroundSparkFade(row) => WeldWorldRef::GroundSparkFade(row),
            Self::HailChild(row) => WeldWorldRef::HailChild(row.view()),
            Self::HailRockFade(row) => WeldWorldRef::HailRockFade(row),
            Self::Impact(row) => WeldWorldRef::Impact(row),
            Self::Meteor(row) => WeldWorldRef::Meteor(row),
            Self::MeteorFlash(row) => WeldWorldRef::MeteorFlash(row),
            Self::MeteorMarker(row) => WeldWorldRef::MeteorMarker(row),
            Self::Persistent(row) => WeldWorldRef::Persistent(row.view()),
            Self::Steam(row) => WeldWorldRef::Steam(row),
        }
    }
    pub fn common(&self) -> &NativeWeldActorBase {
        self.view().common()
    }
    pub fn common_mut(&mut self) -> &mut NativeWeldActorBase {
        match self {
            Self::BlizzardChainFrost(row) => &mut row.common,
            Self::BlizzardGlow(row) => &mut row.common,
            Self::BoulderDebris(row) => &mut row.common,
            Self::Channel(row) => &mut row.common,
            Self::FlameLashFade(row) => &mut row.common,
            Self::FrostFade(row) => &mut row.common,
            Self::GroundSparkFade(row) => &mut row.common,
            Self::HailChild(row) => row.common_mut(),
            Self::HailRockFade(row) => &mut row.common,
            Self::Impact(row) => &mut row.common,
            Self::Meteor(row) => &mut row.common,
            Self::MeteorFlash(row) => &mut row.common,
            Self::MeteorMarker(row) => &mut row.common,
            Self::Persistent(row) => row.common_mut(),
            Self::Steam(row) => &mut row.common,
        }
    }
    pub fn replace(&mut self, record: NativeWeldWorldActor) {
        match (self, record) {
            (
                Self::BlizzardChainFrost(stored),
                NativeWeldWorldActor::BlizzardChainFrost(record),
            ) => **stored = record,
            (Self::BlizzardGlow(stored), NativeWeldWorldActor::BlizzardGlow(record)) => {
                **stored = record
            }
            (Self::BoulderDebris(stored), NativeWeldWorldActor::BoulderDebris(record)) => {
                **stored = record
            }
            (Self::Channel(stored), NativeWeldWorldActor::Channel(record)) => **stored = record,
            (Self::FlameLashFade(stored), NativeWeldWorldActor::FlameLashFade(record)) => {
                **stored = record
            }
            (Self::FrostFade(stored), NativeWeldWorldActor::FrostFade(record)) => **stored = record,
            (Self::GroundSparkFade(stored), NativeWeldWorldActor::GroundSparkFade(record)) => {
                **stored = record
            }
            (Self::HailChild(stored), NativeWeldWorldActor::HailChild(record)) => {
                stored.replace(record)
            }
            (Self::HailRockFade(stored), NativeWeldWorldActor::HailRockFade(record)) => {
                **stored = record
            }
            (Self::Impact(stored), NativeWeldWorldActor::Impact(record)) => **stored = record,
            (Self::Meteor(stored), NativeWeldWorldActor::Meteor(record)) => **stored = record,
            (Self::MeteorFlash(stored), NativeWeldWorldActor::MeteorFlash(record)) => {
                **stored = record
            }
            (Self::MeteorMarker(stored), NativeWeldWorldActor::MeteorMarker(record)) => {
                **stored = record
            }
            (Self::Persistent(stored), NativeWeldWorldActor::Persistent(record)) => {
                stored.replace(record)
            }
            (Self::Steam(stored), NativeWeldWorldActor::Steam(record)) => **stored = record,
            _ => panic!("retained effect keeps its component type"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Serialize),
    serde(rename = "NativeWeldPersistentActorState")
)]
pub enum WeldPersistentRef<'a> {
    EtherealBoulder(&'a NativeWeldEtherealBoulderState),
    Hailstones(&'a NativeWeldHailstonesState),
    MeteorField(&'a NativeWeldMeteorFieldState),
}

pub enum WeldPersistentMut<'a> {
    EtherealBoulder(&'a mut NativeWeldEtherealBoulderState),
    Hailstones(&'a mut NativeWeldHailstonesState),
    MeteorField(&'a mut NativeWeldMeteorFieldState),
}
impl NativeWeldPersistentActorState {
    pub fn view(&self) -> WeldPersistentRef<'_> {
        match self {
            Self::EtherealBoulder(row) => WeldPersistentRef::EtherealBoulder(row),
            Self::Hailstones(row) => WeldPersistentRef::Hailstones(row),
            Self::MeteorField(row) => WeldPersistentRef::MeteorField(row),
        }
    }
    pub fn view_mut(&mut self) -> WeldPersistentMut<'_> {
        match self {
            Self::EtherealBoulder(row) => WeldPersistentMut::EtherealBoulder(row),
            Self::Hailstones(row) => WeldPersistentMut::Hailstones(row),
            Self::MeteorField(row) => WeldPersistentMut::MeteorField(row),
        }
    }
}
impl<'a> WeldPersistentRef<'a> {
    pub fn common(self) -> &'a NativeWeldActorBase {
        match self {
            Self::EtherealBoulder(row) => &row.common,
            Self::Hailstones(row) => &row.common,
            Self::MeteorField(row) => &row.common,
        }
    }
    pub fn snapshot(self) -> NativeWeldPersistentActorState {
        match self {
            Self::EtherealBoulder(row) => {
                NativeWeldPersistentActorState::EtherealBoulder(row.clone())
            }
            Self::Hailstones(row) => NativeWeldPersistentActorState::Hailstones(row.clone()),
            Self::MeteorField(row) => NativeWeldPersistentActorState::MeteorField(row.clone()),
        }
    }
}
impl WeldPersistentMut<'_> {
    pub fn view(&self) -> WeldPersistentRef<'_> {
        match self {
            Self::EtherealBoulder(row) => WeldPersistentRef::EtherealBoulder(row),
            Self::Hailstones(row) => WeldPersistentRef::Hailstones(row),
            Self::MeteorField(row) => WeldPersistentRef::MeteorField(row),
        }
    }
    pub fn common(&self) -> &NativeWeldActorBase {
        self.view().common()
    }
    pub fn common_mut(&mut self) -> &mut NativeWeldActorBase {
        match self {
            Self::EtherealBoulder(row) => &mut row.common,
            Self::Hailstones(row) => &mut row.common,
            Self::MeteorField(row) => &mut row.common,
        }
    }
    pub fn replace(&mut self, record: NativeWeldPersistentActorState) {
        match (self, record) {
            (
                Self::EtherealBoulder(stored),
                NativeWeldPersistentActorState::EtherealBoulder(record),
            ) => **stored = record,
            (Self::Hailstones(stored), NativeWeldPersistentActorState::Hailstones(record)) => {
                **stored = record
            }
            (Self::MeteorField(stored), NativeWeldPersistentActorState::MeteorField(record)) => {
                **stored = record
            }
            _ => panic!("retained effect keeps its component type"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Serialize),
    serde(rename = "NativeWeldHailChildActorState")
)]
pub enum WeldHailChildRef<'a> {
    Flash(&'a NativeWeldHailFlashState),
    Knockback(&'a NativeWeldHailKnockbackState),
    Line(&'a NativeWeldHailLineState),
    TerrainBouncer(&'a NativeWeldHailTerrainBouncerState),
    TerrainParticle(&'a NativeWeldHailTerrainParticleState),
}

pub enum WeldHailChildMut<'a> {
    Flash(&'a mut NativeWeldHailFlashState),
    Knockback(&'a mut NativeWeldHailKnockbackState),
    Line(&'a mut NativeWeldHailLineState),
    TerrainBouncer(&'a mut NativeWeldHailTerrainBouncerState),
    TerrainParticle(&'a mut NativeWeldHailTerrainParticleState),
}
impl NativeWeldHailChildActorState {
    pub fn view(&self) -> WeldHailChildRef<'_> {
        match self {
            Self::Flash(row) => WeldHailChildRef::Flash(row),
            Self::Knockback(row) => WeldHailChildRef::Knockback(row),
            Self::Line(row) => WeldHailChildRef::Line(row),
            Self::TerrainBouncer(row) => WeldHailChildRef::TerrainBouncer(row),
            Self::TerrainParticle(row) => WeldHailChildRef::TerrainParticle(row),
        }
    }
    pub fn view_mut(&mut self) -> WeldHailChildMut<'_> {
        match self {
            Self::Flash(row) => WeldHailChildMut::Flash(row),
            Self::Knockback(row) => WeldHailChildMut::Knockback(row),
            Self::Line(row) => WeldHailChildMut::Line(row),
            Self::TerrainBouncer(row) => WeldHailChildMut::TerrainBouncer(row),
            Self::TerrainParticle(row) => WeldHailChildMut::TerrainParticle(row),
        }
    }
}
impl<'a> WeldHailChildRef<'a> {
    pub fn common(self) -> &'a NativeWeldActorBase {
        match self {
            Self::Flash(row) => &row.common,
            Self::Knockback(row) => &row.common,
            Self::Line(row) => &row.common,
            Self::TerrainBouncer(row) => &row.common,
            Self::TerrainParticle(row) => &row.common,
        }
    }
    pub fn snapshot(self) -> NativeWeldHailChildActorState {
        match self {
            Self::Flash(row) => NativeWeldHailChildActorState::Flash(row.clone()),
            Self::Knockback(row) => NativeWeldHailChildActorState::Knockback(row.clone()),
            Self::Line(row) => NativeWeldHailChildActorState::Line(row.clone()),
            Self::TerrainBouncer(row) => NativeWeldHailChildActorState::TerrainBouncer(row.clone()),
            Self::TerrainParticle(row) => {
                NativeWeldHailChildActorState::TerrainParticle(row.clone())
            }
        }
    }
}
impl WeldHailChildMut<'_> {
    pub fn view(&self) -> WeldHailChildRef<'_> {
        match self {
            Self::Flash(row) => WeldHailChildRef::Flash(row),
            Self::Knockback(row) => WeldHailChildRef::Knockback(row),
            Self::Line(row) => WeldHailChildRef::Line(row),
            Self::TerrainBouncer(row) => WeldHailChildRef::TerrainBouncer(row),
            Self::TerrainParticle(row) => WeldHailChildRef::TerrainParticle(row),
        }
    }
    pub fn common(&self) -> &NativeWeldActorBase {
        self.view().common()
    }
    pub fn common_mut(&mut self) -> &mut NativeWeldActorBase {
        match self {
            Self::Flash(row) => &mut row.common,
            Self::Knockback(row) => &mut row.common,
            Self::Line(row) => &mut row.common,
            Self::TerrainBouncer(row) => &mut row.common,
            Self::TerrainParticle(row) => &mut row.common,
        }
    }
    pub fn replace(&mut self, record: NativeWeldHailChildActorState) {
        match (self, record) {
            (Self::Flash(stored), NativeWeldHailChildActorState::Flash(record)) => {
                **stored = record
            }
            (Self::Knockback(stored), NativeWeldHailChildActorState::Knockback(record)) => {
                **stored = record
            }
            (Self::Line(stored), NativeWeldHailChildActorState::Line(record)) => **stored = record,
            (
                Self::TerrainBouncer(stored),
                NativeWeldHailChildActorState::TerrainBouncer(record),
            ) => **stored = record,
            (
                Self::TerrainParticle(stored),
                NativeWeldHailChildActorState::TerrainParticle(record),
            ) => **stored = record,
            _ => panic!("retained effect keeps its component type"),
        }
    }
}

impl WeldPersistentRef<'_> {
    pub fn held(self) -> bool {
        match self {
            Self::EtherealBoulder(actor) => actor.phase == PrimarySpellProjectilePhase::Held,
            Self::Hailstones(actor) => actor.phase == PrimarySpellProjectilePhase::Held,
            Self::MeteorField(_) => true,
        }
    }
}
impl WeldPersistentMut<'_> {
    pub fn held(&self) -> bool {
        self.view().held()
    }
}

impl StaffActionRef<'_> {
    pub fn base(&self) -> &NativePlayerStaffActionBase {
        match self {
            Self::Melee(actor) => &actor.base,
            Self::Spin(actor) => &actor.base,
        }
    }
}
impl StaffActionMut<'_> {
    pub fn base(&self) -> &NativePlayerStaffActionBase {
        match self {
            Self::Melee(actor) => &actor.base,
            Self::Spin(actor) => &actor.base,
        }
    }
    pub fn base_mut(&mut self) -> &mut NativePlayerStaffActionBase {
        match self {
            Self::Melee(actor) => &mut actor.base,
            Self::Spin(actor) => &mut actor.base,
        }
    }
}
impl StaffTransientRef<'_> {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Action(StaffActionRef::Melee(_)) => "player-staff-melee",
            Self::Action(StaffActionRef::Spin(_)) => "player-staff-spin",
            Self::Contact(_) => "player-staff-contact",
            Self::ContactKnockback(_) => "player-staff-contact-knockback",
            Self::PikeBreak(_) => "player-staff-pike-break",
            Self::Vfx(StaffVfxRef::Smoke(_)) => "player-staff-smoke",
            Self::Vfx(StaffVfxRef::MoveFade(_)) => "player-staff-move-fade",
            Self::Vfx(StaffVfxRef::PerspectiveFade(_)) => "player-staff-perspective-fade",
            Self::Knockback(_) => "player-staff-knockback",
        }
    }
}
