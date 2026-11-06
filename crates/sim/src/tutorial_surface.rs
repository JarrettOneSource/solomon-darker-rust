//! UI surface actions that the Tutorial reacts to, such as opening Inventory or Skills,
//! shared by the UI and the authority.

/// A UI surface action the Tutorial can wait for: opening or closing a panel, or opening a
/// selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum TutorialSurfaceAction {
    InventoryOpened,
    InventoryClosed,
    SkillsOpened,
    SkillsClosed,
    PrimarySelectorOpened,
    ConcentrationASelectorOpened,
}

impl TutorialSurfaceAction {
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
