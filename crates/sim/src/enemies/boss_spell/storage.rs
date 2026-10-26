//! Boss spell ownership and programs, including child-before-parent admission.

use std::fmt;

use bevy_ecs::query::QueryItem;
use bevy_ecs::world::EntityRef;

use super::{BossSpellKind, BossSpellOwner, NativeBossSpell};
use crate::ecs::{EntityStep, OrderedEntities};

pub trait BossSpellRead {
    fn view(&self) -> BossSpellRef<'_>;

    fn snapshot(&self) -> NativeBossSpell {
        let spell = self.view();
        NativeBossSpell { owner: *spell.owner, kind: spell.kind.clone() }
    }
}

impl BossSpellRead for NativeBossSpell {
    fn view(&self) -> BossSpellRef<'_> {
        BossSpellRef { owner: &self.owner, kind: &self.kind }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize), serde(rename = "NativeBossSpell"))]
pub struct BossSpellRef<'a> {
    pub owner: &'a BossSpellOwner,
    pub kind: &'a BossSpellKind,
}

impl BossSpellRead for BossSpellRef<'_> {
    fn view(&self) -> BossSpellRef<'_> {
        *self
    }
}

pub struct BossSpellMut<'a> {
    pub owner: &'a mut BossSpellOwner,
    pub kind: &'a mut BossSpellKind,
}

impl BossSpellRead for BossSpellMut<'_> {
    fn view(&self) -> BossSpellRef<'_> {
        BossSpellRef { owner: self.owner, kind: self.kind }
    }
}

type BossSpellQuery = (&'static mut BossSpellOwner, &'static mut BossSpellKind);

impl<'a> BossSpellMut<'a> {
    fn from_query((owner, kind): QueryItem<'a, 'static, BossSpellQuery>) -> Self {
        Self { owner: owner.into_inner(), kind: kind.into_inner() }
    }
}

#[derive(Default)]
pub struct BossSpells {
    entities: OrderedEntities<BossSpellQuery>,
    births: Option<Vec<NativeBossSpell>>,
}

impl BossSpells {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = BossSpellRef<'_>> + ExactSizeIterator + Clone {
        self.entities.iter().map(Self::read)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = BossSpellMut<'_>> {
        self.entities.iter_mut().map(BossSpellMut::from_query)
    }

    fn read(entity: EntityRef<'_>) -> BossSpellRef<'_> {
        BossSpellRef {
            owner: entity.get::<BossSpellOwner>().expect("boss spell owns its identity and clock"),
            kind: entity.get::<BossSpellKind>().expect("boss spell owns its program"),
        }
    }

    pub fn clear(&mut self) {
        self.entities.clear();
        if let Some(births) = &mut self.births {
            births.clear();
        }
    }

    pub fn push(&mut self, spell: NativeBossSpell) {
        if let Some(births) = &mut self.births {
            births.push(spell);
        } else {
            self.entities.spawn(|world| world.spawn((spell.owner, spell.kind)).id());
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&BossSpellRef<'_>) -> bool) {
        self.entities.retain(|entity| keep(&Self::read(entity)));
    }

    pub(crate) fn collect_births(&mut self) {
        assert!(self.is_empty() && self.births.is_none(), "boss spell births have one phase owner");
        self.births = Some(Vec::new());
    }

    pub(crate) fn take_births(&mut self) -> Vec<NativeBossSpell> {
        std::mem::take(self.births.as_mut().expect("boss spell phase collects its births"))
    }

    pub(crate) fn advance(
        &mut self,
        mut step: impl FnMut(&mut BossSpellMut<'_>) -> (bool, Vec<NativeBossSpell>),
    ) {
        self.entities.advance_with_births(
            |components| {
                let (retained, births_before) = step(&mut BossSpellMut::from_query(components));
                EntityStep { retained, births_before, births_after: Vec::new() }
            },
            |world, spell| world.spawn((spell.owner, spell.kind)).id(),
        );
    }
}

impl FromIterator<NativeBossSpell> for BossSpells {
    fn from_iter<I: IntoIterator<Item = NativeBossSpell>>(spells: I) -> Self {
        let mut result = Self::default();
        for spell in spells {
            result.push(spell);
        }
        result
    }
}

impl Clone for BossSpells {
    fn clone(&self) -> Self {
        let mut result: Self = self.iter().map(|spell| spell.snapshot()).collect();
        result.births.clone_from(&self.births);
        result
    }
}

impl fmt::Debug for BossSpells {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for BossSpells {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter()) && self.births == other.births
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for BossSpells {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for BossSpells {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<NativeBossSpell>::deserialize(deserializer).map(|spells| spells.into_iter().collect())
    }
}
