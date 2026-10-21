//! Native wizard class names and elemental fallback from `core-kernels/native-wizard-class.ts`.

use super::{WizardDiscipline, WizardElement};

#[must_use]
pub const fn native_wizard_class_title(
    element: WizardElement,
    discipline: WizardDiscipline,
) -> &'static str {
    use WizardDiscipline::{Arcane, Body, Mind};
    use WizardElement::{Air, Earth, Ether, Fire, Water};

    match (element, discipline) {
        (
            _,
            WizardDiscipline::Air
            | WizardDiscipline::Earth
            | WizardDiscipline::Ether
            | WizardDiscipline::Fire
            | WizardDiscipline::Water,
        ) => "WIZARD",
        (Air, Arcane) => "STORM MAGE",
        (Air, Body) => "STORMCALLER",
        (Air, Mind) => "ASTROLOGER",
        (Earth, Arcane) => "EARTH MAGE",
        (Earth, Body) => "RITUALIST",
        (Earth, Mind) => "CHANNELER",
        (Ether, Arcane) => "OCCULTIST",
        (Ether, Body) => "SAGE",
        (Ether, Mind) => "SEER",
        (Fire, Arcane) => "FIRE MAGE",
        (Fire, Body) => "WARLOCK",
        (Fire, Mind) => "PYROMANCER",
        (Water, Arcane) => "FROST MAGE",
        (Water, Body) => "ICEBINDER",
        (Water, Mind) => "THAUMATURGE",
    }
}
