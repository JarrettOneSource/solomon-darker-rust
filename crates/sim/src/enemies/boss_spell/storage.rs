//! Boss spells in admission order, including child-before-parent admission.

use super::NativeBossSpell;

/// Live boss spells in admission order. While a phase collects births, spells
/// pushed during that phase wait in `births` until the phase admits them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BossSpells {
    spells: Vec<NativeBossSpell>,
    births: Option<Vec<NativeBossSpell>>,
}

impl BossSpells {
    pub fn len(&self) -> usize {
        self.spells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spells.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, NativeBossSpell> {
        self.spells.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, NativeBossSpell> {
        self.spells.iter_mut()
    }

    pub fn clear(&mut self) {
        self.spells.clear();
        if let Some(births) = &mut self.births {
            births.clear();
        }
    }

    /// Admits a spell, or holds it for the phase that is collecting births.
    pub fn push(&mut self, spell: NativeBossSpell) {
        match &mut self.births {
            Some(births) => births.push(spell),
            None => self.spells.push(spell),
        }
    }

    pub fn retain(&mut self, keep: impl FnMut(&NativeBossSpell) -> bool) {
        self.spells.retain(keep);
    }

    pub(crate) fn collect_births(&mut self) {
        assert!(self.is_empty() && self.births.is_none(), "boss spell births have one phase owner");
        self.births = Some(Vec::new());
    }

    pub(crate) fn take_births(&mut self) -> Vec<NativeBossSpell> {
        std::mem::take(self.births.as_mut().expect("boss spell phase collects its births"))
    }

    /// Steps each spell once, in admission order. A spell's births enter just
    /// ahead of it and take their first step on the next pass.
    pub(crate) fn advance(
        &mut self,
        mut step: impl FnMut(&mut NativeBossSpell) -> (bool, Vec<NativeBossSpell>),
    ) {
        let population = std::mem::take(&mut self.spells);
        self.spells.reserve(population.len());
        for mut spell in population {
            let (retained, births) = step(&mut spell);
            self.spells.extend(births);
            if retained {
                self.spells.push(spell);
            }
        }
    }
}

impl FromIterator<NativeBossSpell> for BossSpells {
    fn from_iter<I: IntoIterator<Item = NativeBossSpell>>(spells: I) -> Self {
        Self { spells: spells.into_iter().collect(), births: None }
    }
}

// Births only exist inside a phase, so saves hold the admitted spells alone.
#[cfg(feature = "persistence")]
impl serde::Serialize for BossSpells {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(&self.spells)
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for BossSpells {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::deserialize(deserializer).map(|spells| Self { spells, births: None })
    }
}
