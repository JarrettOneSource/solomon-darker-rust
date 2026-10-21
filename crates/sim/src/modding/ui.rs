//! Borrowed frontend projections from Website `mod-ui/`. Accepted content and
//! runtime rows stay with their existing host/client owners.

mod minimap;
mod skills;

pub use minimap::{
    ModMinimapEnemy, ModMinimapMarker, ModMinimapMarkerId, ModMinimapMarkerKind, ModMinimapModel,
    ModMinimapPlayer, ModMinimapSnapshot, project_mod_minimap,
};
pub use skills::{
    ModLearnedSkill, ModOfferedSkill, ModSkillBookModel, ModSkillIcon, ModSkillPickerModel,
    ModSpellBinding, project_mod_skill_book, project_mod_skill_picker,
};
