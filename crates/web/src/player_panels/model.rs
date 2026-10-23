//! Owned presentation inputs for the later GPU paint pass. Inventory and authored
//! catalogues refresh only when their accepted revisions change.
use super::*;

pub(super) struct PanelModel {
    selection: NewGameSelection,
    book: NativeSkillBookState,
    stat_book: PlayerStatBookComponent,
    economy: HubEconomyState,
    mod_content: Option<ModContentProjection>,
    belt: PlayerBeltComponent,
    dye_session_id: Option<String>,
    progression: PlayerProgressionState,
    resources: PlayerCombatState,
    offensive: NativeOffensiveSpellFactors,
    attributes: InventoryAttributes,
    concentrations: [Option<usize>; 2],
    mind_chug_active: bool,
    perk_removal_enabled: bool,
    kills: u64,
    awesomeness: u64,
}
impl PanelModel {
    pub fn refresh(previous: Option<Self>, s: &PlayerPanelSnapshot<'_>) -> Self {
        let Some(mut model) = previous else {
            return Self {
                selection: s.selection.clone(),
                book: s.book.clone(),
                stat_book: s.stat_book.clone(),
                economy: s.economy.clone(),
                mod_content: s.mod_content.cloned(),
                belt: *s.belt,
                dye_session_id: s.dye_session_id.map(str::to_owned),
                progression: s.progression.clone(),
                resources: s.resources.clone(),
                offensive: s.offensive.clone(),
                attributes: s.attributes,
                concentrations: *s.concentrations,
                mind_chug_active: s.mind_chug_active,
                perk_removal_enabled: s.perk_removal_enabled,
                kills: s.kills,
                awesomeness: s.awesomeness,
            };
        };
        if model.economy.revision != s.economy.revision {
            model.economy.clone_from(s.economy);
        }
        if model.selection != *s.selection {
            model.selection.clone_from(s.selection);
        }
        if model.book != *s.book {
            model.book.clone_from(s.book);
        }
        if model.stat_book != *s.stat_book {
            model.stat_book.clone_from(s.stat_book);
        }
        if model.mod_content.as_ref() != s.mod_content {
            model.mod_content = s.mod_content.cloned();
        }
        model.belt = *s.belt;
        if model.dye_session_id.as_deref() != s.dye_session_id {
            model.dye_session_id = s.dye_session_id.map(str::to_owned);
        }
        if model.progression.revision != s.progression.revision {
            model.progression.clone_from(s.progression);
        }
        model.resources.clone_from(s.resources);
        if model.offensive != *s.offensive {
            model.offensive.clone_from(s.offensive);
        }
        model.attributes = s.attributes;
        model.concentrations = *s.concentrations;
        model.mind_chug_active = s.mind_chug_active;
        model.perk_removal_enabled = s.perk_removal_enabled;
        model.kills = s.kills;
        model.awesomeness = s.awesomeness;
        model
    }
    pub fn snapshot(&self) -> PlayerPanelSnapshot<'_> {
        PlayerPanelSnapshot {
            selection: &self.selection,
            book: &self.book,
            stat_book: &self.stat_book,
            economy: &self.economy,
            mod_content: self.mod_content.as_ref(),
            mod_assets: None,
            belt: &self.belt,
            dye_session_id: self.dye_session_id.as_deref(),
            progression: &self.progression,
            resources: &self.resources,
            offensive: &self.offensive,
            attributes: self.attributes,
            concentrations: &self.concentrations,
            mind_chug_active: self.mind_chug_active,
            perk_removal_enabled: self.perk_removal_enabled,
            kills: self.kills,
            awesomeness: self.awesomeness,
        }
    }
}
impl PlayerPanels {
    pub(super) fn refresh_mod_icons(&mut self, s: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        fn visit(
            items: &[solomon_sim::economy::HubInventoryItem],
            images: Option<&crate::mod_presentation_assets::ModPresentationAssets>,
            icons: &mut std::collections::BTreeMap<u64, Vec<crate::ui::mod_icon::ModIconLayer>>,
        ) -> Result<(), String> {
            for item in items {
                if let Some(layers) = crate::ui::mod_icon::mod_item_icon_layers(images, item)? {
                    icons.insert(item.id, layers);
                }
                if let Some(children) = &item.contents {
                    visit(children, images, icons)?;
                }
            }
            Ok(())
        }
        let mut icons = std::collections::BTreeMap::new();
        visit(&s.economy.backpack, s.mod_assets, &mut icons)?;
        visit(&s.economy.storage, s.mod_assets, &mut icons)?;
        for slot in EquipmentSlot::ALL {
            if let Some(item) = s.economy.equipment.at(slot) {
                visit(std::slice::from_ref(item), s.mod_assets, &mut icons)?;
            }
        }
        for stock in &s.economy.fomentius_stock {
            visit(std::slice::from_ref(&stock.item), s.mod_assets, &mut icons)?;
        }
        self.mod_icons = icons;
        self.mod_boast_images.clear();
        if let (Some(mods), Some(images)) = (s.mod_content, s.mod_assets) {
            for boast in &mods.boasts {
                if let solomon_sim::modding::content::PreparedModBoastIcon::Mod {
                    image_path, ..
                } = &boast.icon
                {
                    let image = images.image(&boast.mod_id, image_path)?;
                    self.mod_boast_images.insert(
                        format!("{}:{image_path}", boast.mod_id),
                        (image.url().to_owned(), image.dimensions()),
                    );
                }
            }
        }
        Ok(())
    }
}
