//! Create's missing-choice finalizer from `core-kernels/native-create-loadout.ts`.

use super::{NewGameSelection, WizardDiscipline, WizardElement};
use crate::NativeRng;

/// Retail Create fades by float32 0.05 at 100 Hz.
pub const NATIVE_CREATE_ENTRY_FADE_MS: u64 = 200;
/// Finalization requires the fade to be strictly greater than float32 1.1.
pub const NATIVE_CREATE_RANDOMIZE_FINALIZE_MS: u64 = 230;

const PRIMARY_POOL: [WizardElement; 5] = [
    WizardElement::Ether,
    WizardElement::Fire,
    WizardElement::Air,
    WizardElement::Water,
    WizardElement::Earth,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeCreateLoadoutSelection {
    pub name: String,
    pub element: Option<WizardElement>,
    pub discipline: Option<WizardDiscipline>,
}

/// `finalizeNativeCreateLoadout`: retain explicit choices and draw only missing ones.
#[must_use]
pub fn finalize_native_create_loadout(
    rng: &mut NativeRng,
    selection: &NativeCreateLoadoutSelection,
    disabled_primary_skill_ids: &[usize],
) -> NewGameSelection {
    let element = if let Some(element) = selection.element {
        element
    } else {
        let mut pool = Vec::with_capacity(PRIMARY_POOL.len());
        for (root, element) in PRIMARY_POOL.into_iter().enumerate() {
            if !disabled_primary_skill_ids.contains(&(8 + root * 8)) {
                pool.push(element);
            }
        }
        // Integer(0) consumes no word. Source then reads a zero-initialized slot.
        let index = rng.draw_integer(pool.len() as i32) as usize;
        pool.get(index).copied().unwrap_or(WizardElement::Ether)
    };
    let discipline = if let Some(discipline) = selection.discipline {
        discipline
    } else if rng.draw_integer(3) == 1 {
        // The modulo-biased gate and repeated equal-root draws are intentional;
        // replacing them with a uniform pool changes the subsequent native stream.
        loop {
            let candidate = PRIMARY_POOL[rng.draw_integer(5) as usize];
            if candidate != element {
                break candidate.into();
            }
        }
    } else {
        WizardDiscipline::CHOICES[rng.draw_integer(3) as usize]
    };
    NewGameSelection { name: selection.name.clone(), element, discipline }
}
