//! Tutorial surface intents from `native-tutorial.ts`, shared by UI and authority.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeTutorialSurfaceAction {
    InventoryOpened,
    InventoryClosed,
    SkillsOpened,
    SkillsClosed,
    PrimarySelectorOpened,
    ConcentrationASelectorOpened,
}

impl NativeTutorialSurfaceAction {
    pub const ALL: [Self; 6] = [
        Self::InventoryOpened,
        Self::InventoryClosed,
        Self::SkillsOpened,
        Self::SkillsClosed,
        Self::PrimarySelectorOpened,
        Self::ConcentrationASelectorOpened,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::InventoryOpened => "inventory-opened",
            Self::InventoryClosed => "inventory-closed",
            Self::SkillsOpened => "skills-opened",
            Self::SkillsClosed => "skills-closed",
            Self::PrimarySelectorOpened => "primary-selector-opened",
            Self::ConcentrationASelectorOpened => "concentration-a-selector-opened",
        }
    }
}
