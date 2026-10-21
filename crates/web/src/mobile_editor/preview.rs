//! Source `MobileUiElementPreview` and `MobileUiBeltPreview`, borrowing actual player
//! fields. Diagnostics in the editor are the source's fixed positioning samples.

use solomon_sim::appearance::equipment::native_wizard_icon_primary_tint;
use solomon_sim::belt::{
    NATIVE_BELT_SLOT_COUNT, NativeBeltEntry, NativeBeltPotionKind, native_belt_entry_item,
    native_belt_potion_projection,
};
use solomon_sim::economy::{DOWSING_EQUIPMENT_RECIPES, EquipmentType, HubInventoryItem};
use solomon_sim::hud::quickbar::{
    NativeSkillQuickbarCooldownPresentation, native_cooldown_sector_path,
    native_skill_quickbar_icon_alpha,
};
use solomon_sim::mobile_ui::{MobileUiElementId, mobile_ui_belt_element_id};
use solomon_sim::player::PlayerState;
use solomon_sim::profile::native_skill_category;
use solomon_sim::secondary::{
    NativeSecondaryAbilityId, native_secondary_ability_mana_cost_with_stat_book,
};
use web_sys::HtmlElement;

use super::{MobileUiEditor, append, element};
use crate::player_panels::chrome::{art, tint};

#[derive(Clone, Debug, PartialEq)]
struct ItemIcon {
    id: u64,
    records: Vec<usize>,
    tints: [Option<u32>; 2],
    equipment_type: Option<EquipmentType>,
    quantity: u64,
}

#[derive(Clone, Debug, PartialEq)]
enum BeltIcon {
    Skill {
        id: usize,
        record: usize,
        unavailable: bool,
        cooldown: NativeSkillQuickbarCooldownPresentation,
    },
    Item(ItemIcon),
    Potion {
        mana: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MobileEditorPlayerPreview {
    slots: [Option<BeltIcon>; 8],
    aliases: [Option<&'static str>; 8],
    potions: [BeltIcon; 2],
    in_hub: bool,
}

impl MobileEditorPlayerPreview {
    pub fn from_player(player: &PlayerState, in_hub: bool) -> Self {
        let factors = player.derived_stats().offensive_factors(&player.skill_runtime);
        let mut slots = std::array::from_fn(|_| None);
        let mut aliases = [None; 8];
        // Source MobileUiElementPreview renders the eight named editor controls
        // from bank zero. The live HUD selects its own bank before projecting it;
        // the editor must never enumerate the other sixteen persisted entries.
        for (slot, entry) in player.belt.entries.iter().take(NATIVE_BELT_SLOT_COUNT).enumerate() {
            let id = mobile_ui_belt_element_id(&player.belt, slot);
            if id != MobileUiElementId::SLOTS[slot] {
                aliases[slot] =
                    Some(if id == MobileUiElementId::HealthPotion { "Health" } else { "Mana" });
                continue;
            }
            slots[slot] = match *entry {
                Some(NativeBeltEntry::Skill { skill_id }) => {
                    let ability = NativeSecondaryAbilityId::from_skill_id(skill_id);
                    let unavailable = ability.is_some_and(|ability| {
                        in_hub
                            || player.combat.current_mana
                                < native_secondary_ability_mana_cost_with_stat_book(
                                    &player.book,
                                    &player.stat_book,
                                    ability,
                                    &factors,
                                )
                    });
                    Some(BeltIcon::Skill {
                        id: skill_id,
                        record: player.book.skill_icon_record(skill_id),
                        unavailable,
                        cooldown: NativeSkillQuickbarCooldownPresentation::default(),
                    })
                }
                Some(entry) => {
                    if let Some(item) = native_belt_entry_item(entry, &player.economy) {
                        Some(BeltIcon::Item(item_icon(item, u64::from(item.quantity), player)))
                    } else {
                        match entry {
                            NativeBeltEntry::HealthPotion => Some(BeltIcon::Potion { mana: false }),
                            NativeBeltEntry::ManaPotion => Some(BeltIcon::Potion { mana: true }),
                            _ => None,
                        }
                    }
                }
                None => None,
            };
        }
        let potions = [NativeBeltPotionKind::Health, NativeBeltPotionKind::Mana].map(|kind| {
            let projection = native_belt_potion_projection(&player.economy.backpack, kind);
            projection
                .item
                .map_or(BeltIcon::Potion { mana: kind == NativeBeltPotionKind::Mana }, |item| {
                    BeltIcon::Item(item_icon(item, projection.count, player))
                })
        });
        Self { slots, aliases, potions, in_hub }
    }

    pub fn set_cooldowns(&mut self, cooldowns: &[NativeSkillQuickbarCooldownPresentation; 83]) {
        for icon in self.slots.iter_mut().flatten() {
            if let BeltIcon::Skill { id, cooldown, .. } = icon {
                *cooldown = if native_skill_category(*id) == Some(2) {
                    cooldowns[*id]
                } else {
                    NativeSkillQuickbarCooldownPresentation::default()
                };
            }
        }
    }
}

fn item_icon(item: &HubInventoryItem, quantity: u64, player: &PlayerState) -> ItemIcon {
    ItemIcon {
        id: item.id,
        records: item.icon_records.clone(),
        equipment_type: item.equipment_type,
        quantity,
        tints: if matches!(item.equipment_type, Some(EquipmentType::Hat | EquipmentType::Robe)) {
            item.icon_tints
                .or_else(|| {
                    item.recipe_index
                        .and_then(|index| DOWSING_EQUIPMENT_RECIPES.get(index))
                        .map(|recipe| recipe.icon_tints)
                })
                .unwrap_or([
                    Some(native_wizard_icon_primary_tint(player.selection.element)),
                    Some(0xffffff),
                ])
        } else {
            [None, None]
        },
    }
}

impl MobileUiEditor {
    pub(super) fn paint_previews(&self) -> Result<(), String> {
        for id in MobileUiElementId::ALL {
            let root = &self.preview_nodes[id as usize];
            root.set_text_content(None);
            match id {
                MobileUiElementId::Pause => self.preview_art(root, 42, 36.0 / 44.0)?,
                MobileUiElementId::Inventory => self.preview_art(root, 47, 116.0 / 130.0)?,
                MobileUiElementId::Skillbook => self.preview_art(root, 48, 116.0 / 130.0)?,
                MobileUiElementId::Diagnostics => {
                    let diagnostics =
                        element(&self.document, "span", "mobile-ui-editor-diagnostics")?;
                    for text in ["60 FPS", "0 ms"] {
                        append_label(self, &diagnostics, "span", "", text)?;
                    }
                    append(root, &diagnostics)?;
                }
                MobileUiElementId::Meters => {
                    let meters = element(&self.document, "span", "mobile-ui-editor-meters")?;
                    for (class, asset) in [("health", "red"), ("mana", "blue")] {
                        let meter = element(
                            &self.document,
                            "span",
                            &format!("mobile-ui-editor-meter {class}"),
                        )?;
                        // Source uses these retained stock-derived bar composites.
                        // A horizontal strip keeps native pixel height and cannot be
                        // scaled to the editor's percentage-sized image box.
                        let fill = element(&self.document, "img", "")?;
                        fill.set_attribute("src", &format!("assets/stock/hub-hud-bar-{asset}.png"))
                            .map_err(|_| "mobile meter source")?;
                        fill.set_attribute("alt", "").map_err(|_| "mobile meter decoration")?;
                        fill.set_attribute("draggable", "false")
                            .map_err(|_| "mobile meter drag admission")?;
                        append(&meter, &fill)?;
                        append(&meters, &meter)?;
                    }
                    append(root, &meters)?;
                }
                MobileUiElementId::LeftJoystick | MobileUiElementId::RightJoystick => {
                    let base = element(&self.document, "span", "mobile-ui-editor-joystick")?;
                    append(&base, &element(&self.document, "span", "")?)?;
                    let label = append_label(
                        self,
                        &base,
                        "span",
                        "mobile-ui-editor-joystick-label",
                        if id == MobileUiElementId::LeftJoystick { "MOVE" } else { "AIM / CAST" },
                    )?;
                    label
                        .style()
                        .set_property("font-size", &format!("{}px", 11.0 * self.ui_scale))
                        .map_err(|_| "mobile joystick preview label size")?;
                    append(root, &base)?;
                }
                MobileUiElementId::Xp => {
                    let xp = element(&self.document, "span", "mobile-ui-editor-xp")?;
                    for record in [81, 82] {
                        let sprite = self.art.ui.sprite(&self.document, record, "")?;
                        sprite
                            .style()
                            .set_property("position", "absolute")
                            .map_err(|_| "mobile XP preview positioning")?;
                        sprite
                            .style()
                            .set_property("inset", "0")
                            .map_err(|_| "mobile XP preview origin")?;
                        let record = self.art.ui.record(record)?;
                        sprite
                            .style()
                            .set_property(
                                "transform",
                                &format!(
                                    "scale({},{})",
                                    self.geometry.sizes[id as usize].width
                                        / f64::from(record.logical_width),
                                    self.geometry.sizes[id as usize].height
                                        / f64::from(record.logical_height)
                                ),
                            )
                            .map_err(|_| "mobile XP preview scale")?;
                        sprite
                            .style()
                            .set_property("transform-origin", "top left")
                            .map_err(|_| "mobile XP preview scale origin")?;
                        append(&xp, &sprite)?;
                    }
                    append(root, &xp)?;
                }
                MobileUiElementId::HealthPotion | MobileUiElementId::ManaPotion => {
                    let mana = id == MobileUiElementId::ManaPotion;
                    let fallback = BeltIcon::Potion { mana };
                    let icon = self
                        .preview
                        .as_ref()
                        .map_or(&fallback, |preview| &preview.potions[usize::from(mana)]);
                    self.preview_belt(root, Some(icon))?;
                }
                _ => {
                    let slot = MobileUiElementId::SLOTS
                        .iter()
                        .position(|slot| *slot == id)
                        .expect("remaining editor elements are belt slots");
                    let icon =
                        self.preview.as_ref().and_then(|preview| preview.slots[slot].as_ref());
                    self.preview_belt(root, icon)?;
                    append_label(
                        self,
                        root,
                        "span",
                        "mobile-ui-editor-slot-number",
                        &(slot + 1).to_string(),
                    )?;
                    if let Some(alias) =
                        self.preview.as_ref().and_then(|preview| preview.aliases[slot])
                    {
                        let label =
                            append_label(self, root, "span", "mobile-ui-editor-slot-alias", alias)?;
                        append(&label, &element(&self.document, "br", "")?)?;
                        append_label(self, &label, "span", "", "control")?;
                    }
                }
            }
        }
        Ok(())
    }

    fn preview_art(
        &self,
        root: &HtmlElement,
        record: usize,
        width_fraction: f64,
    ) -> Result<(), String> {
        let definition = self.art.ui.record(record)?;
        let id = root
            .get_attribute("data-preview-element")
            .and_then(|id| MobileUiElementId::from_id(&id))
            .ok_or("mobile preview identity")?;
        let scale = self.geometry.sizes[id as usize].width * width_fraction
            / f64::from(definition.logical_width);
        let sprite = self.art.ui.sprite(&self.document, record, "")?;
        sprite
            .style()
            .set_property("transform", &format!("scale({scale})"))
            .map_err(|_| "mobile preview art scale")?;
        append(root, &sprite)
    }

    fn preview_belt(&self, root: &HtmlElement, icon: Option<&BeltIcon>) -> Result<(), String> {
        let slot =
            element(&self.document, "span", "hub-hud-quickbar-slot mobile-ui-editor-belt-preview")?;
        slot.set_attribute("data-populated", if icon.is_some() { "true" } else { "false" })
            .map_err(|_| "mobile belt population")?;
        slot.set_attribute(
            "data-preview-in-hub",
            if self.preview.as_ref().is_some_and(|preview| preview.in_hub) {
                "true"
            } else {
                "false"
            },
        )
        .map_err(|_| "mobile belt Hub visibility")?;
        let scale = self.geometry.sizes[MobileUiElementId::LeftJoystick as usize].width / 237.5;
        let id = root
            .get_attribute("data-preview-element")
            .and_then(|id| MobileUiElementId::from_id(&id))
            .ok_or("mobile belt preview identity")?;
        let size = self.geometry.sizes[id as usize];
        slot.style().set_css_text(&format!("position:absolute;left:50%;top:50%;width:{}px;height:{}px;transform:translate(-50%,-50%) scale({scale});pointer-events:none",
            size.width/scale,size.height/scale));
        match icon {
            Some(BeltIcon::Skill { record, unavailable, cooldown, .. }) => {
                if cooldown.remaining > 0.0 {
                    let svg = self
                        .document
                        .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
                        .map_err(|_| "mobile cooldown SVG")?;
                    svg.set_attribute("viewBox", "0 0 53 53")
                        .map_err(|_| "mobile cooldown square")?;
                    svg.set_attribute("class", "mobile-ui-editor-cooldown")
                        .map_err(|_| "mobile cooldown class")?;
                    let path = self
                        .document
                        .create_element_ns(Some("http://www.w3.org/2000/svg"), "path")
                        .map_err(|_| "mobile cooldown path")?;
                    path.set_attribute(
                        "d",
                        &native_cooldown_sector_path(cooldown.remaining, cooldown.capacity),
                    )
                    .map_err(|_| "mobile cooldown sector")?;
                    svg.append_child(&path).map_err(|_| "mobile cooldown path attachment")?;
                    slot.append_child(&svg).map_err(|_| "mobile cooldown attachment")?;
                }
                let sprite = art(
                    &self.document,
                    &slot,
                    &self.art.skills,
                    *record,
                    [size.width / scale / 2.0, size.height / scale / 2.0],
                    [1.0, 1.0],
                    0.0,
                )?;
                sprite.set_class_name("panel-art hub-hud-quickbar-skill-icon");
                sprite
                    .style()
                    .set_property(
                        "opacity",
                        &native_skill_quickbar_icon_alpha(cooldown.remaining > 0.0, *unavailable)
                            .to_string(),
                    )
                    .map_err(|_| "mobile skill opacity")?;
            }
            Some(BeltIcon::Item(item)) => {
                let (translation, rotation) = match item.equipment_type {
                    Some(EquipmentType::Amulet) => ([0.0, -5.0], 0.0),
                    Some(EquipmentType::Staff) => ([-22.94306, 32.76608], 35.0),
                    Some(EquipmentType::Wand) => ([0.0, 0.0], 45.0),
                    _ => ([0.0, 0.0], 0.0),
                };
                for (index, record) in item.records.iter().enumerate() {
                    let sprite = art(
                        &self.document,
                        &slot,
                        &self.art.inventory,
                        *record,
                        [
                            size.width / scale / 2.0 + translation[0],
                            size.height / scale / 2.0 + translation[1],
                        ],
                        [1.0, 1.0],
                        rotation,
                    )?;
                    if let Some(Some(color)) = item.tints.get(index) {
                        tint(&self.document, &sprite, *color)?;
                    }
                }
                if item.quantity > 1 {
                    append_label(
                        self,
                        &slot,
                        "span",
                        "mobile-ui-editor-item-count",
                        &item.quantity.to_string(),
                    )?;
                }
            }
            Some(BeltIcon::Potion { mana }) => {
                art(
                    &self.document,
                    &slot,
                    &self.art.inventory,
                    if *mana { 47 } else { 46 },
                    [size.width / scale / 2.0, size.height / scale / 2.0],
                    [1.0, 1.0],
                    0.0,
                )?;
            }
            None => {}
        }
        append(root, &slot)
    }
}

fn append_label(
    editor: &MobileUiEditor,
    parent: &HtmlElement,
    tag: &str,
    class: &str,
    text: &str,
) -> Result<HtmlElement, String> {
    let node = element(&editor.document, tag, class)?;
    node.set_text_content(Some(text));
    append(parent, &node)?;
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::NativeRng;
    use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};

    #[test]
    fn current_three_bank_player_keeps_editor_on_first_eight_controls() {
        let mut player = PlayerState::new(
            NewGameSelection {
                name: "Felix".into(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
            &mut NativeRng::new(123),
        );
        assert_eq!(player.belt.entries.len(), 24);
        player.belt.entries.fill(None);
        player.belt.entries[0] = Some(NativeBeltEntry::Skill { skill_id: 8 });
        player.belt.entries[3] = Some(NativeBeltEntry::HealthPotion);
        let before = MobileEditorPlayerPreview::from_player(&player, true);
        assert!(matches!(before.slots[0], Some(BeltIcon::Skill { id: 8, .. })));
        assert_eq!(before.aliases[3], Some("Health"));
        for entry in &mut player.belt.entries[NATIVE_BELT_SLOT_COUNT..] {
            *entry = Some(NativeBeltEntry::ManaPotion);
        }
        assert_eq!(MobileEditorPlayerPreview::from_player(&player, true), before);
    }
}
