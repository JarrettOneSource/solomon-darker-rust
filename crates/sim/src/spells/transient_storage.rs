//! Primary spell transients: impacts, fades and persistent spell actors that
//! share one update and painter order.

use crate::spells::model::PrimarySpellTransient;

/// Live transients in update order. While a phase collects births, effects
/// pushed during that phase wait in `births` until the phase places them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrimaryTransients {
    effects: Vec<PrimarySpellTransient>,
    births: Option<Vec<PrimarySpellTransient>>,
}

impl PrimaryTransients {
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, PrimarySpellTransient> {
        self.effects.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, PrimarySpellTransient> {
        self.effects.iter_mut()
    }

    pub fn get(&self, index: usize) -> Option<&PrimarySpellTransient> {
        self.effects.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut PrimarySpellTransient> {
        self.effects.get_mut(index)
    }

    /// Admits an effect, or holds it for the phase that is collecting births.
    pub fn push(&mut self, effect: PrimarySpellTransient) {
        match &mut self.births {
            Some(births) => births.push(effect),
            None => self.effects.push(effect),
        }
    }

    /// Moves every effect out of `other`, keeping their order.
    pub fn append(&mut self, other: &mut Self) {
        self.extend(std::mem::take(&mut other.effects));
    }

    pub fn retain(&mut self, keep: impl FnMut(&PrimarySpellTransient) -> bool) {
        self.effects.retain(keep);
    }

    pub fn retain_mut(&mut self, keep: impl FnMut(&mut PrimarySpellTransient) -> bool) {
        self.effects.retain_mut(keep);
    }

    /// The sort is stable: effects with equal keys keep their order.
    pub fn sort_by_key<K: Ord>(&mut self, key: impl FnMut(&PrimarySpellTransient) -> K) {
        self.effects.sort_by_key(key);
    }

    pub(crate) fn collect_births(&mut self) {
        assert!(self.is_empty() && self.births.is_none(), "transient births have one phase owner");
        self.births = Some(Vec::new());
    }

    pub(crate) fn take_births(&mut self) -> Vec<PrimarySpellTransient> {
        std::mem::take(self.births.as_mut().expect("transient phase collects its births"))
    }

    /// Steps each effect once, in order. `step` reports whether the effect stays
    /// and returns the effects it created; they enter just behind it and take
    /// their first step on the next pass.
    pub(crate) fn advance(
        &mut self,
        mut step: impl FnMut(&mut PrimarySpellTransient) -> (bool, Vec<PrimarySpellTransient>),
    ) {
        let population = std::mem::take(&mut self.effects);
        self.effects.reserve(population.len());
        for mut effect in population {
            let (retained, births) = step(&mut effect);
            if retained {
                self.effects.push(effect);
            }
            self.effects.extend(births);
        }
    }
}

impl Extend<PrimarySpellTransient> for PrimaryTransients {
    fn extend<I: IntoIterator<Item = PrimarySpellTransient>>(&mut self, effects: I) {
        match &mut self.births {
            Some(births) => births.extend(effects),
            None => self.effects.extend(effects),
        }
    }
}

impl FromIterator<PrimarySpellTransient> for PrimaryTransients {
    fn from_iter<I: IntoIterator<Item = PrimarySpellTransient>>(effects: I) -> Self {
        Self { effects: effects.into_iter().collect(), births: None }
    }
}

impl IntoIterator for PrimaryTransients {
    type Item = PrimarySpellTransient;
    type IntoIter = std::vec::IntoIter<PrimarySpellTransient>;

    fn into_iter(self) -> Self::IntoIter {
        self.effects.into_iter()
    }
}

// Births only exist inside a phase, so saves hold the admitted effects alone.
#[cfg(feature = "persistence")]
impl serde::Serialize for PrimaryTransients {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(&self.effects)
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for PrimaryTransients {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::deserialize(deserializer).map(|effects| Self { effects, births: None })
    }
}
