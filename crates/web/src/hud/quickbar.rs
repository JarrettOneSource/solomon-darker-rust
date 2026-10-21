//! Source `SkillQuickbar.tsx`: live accepted player fields become display-only
//! labels, cooldowns and active flags. Mana formulas stay in the simulation owner.

use solomon_sim::belt::{
    NATIVE_BELT_SLOT_COUNT, NativeBeltEntry, NativeBeltPotionKind, native_belt_entry_item,
    native_belt_potion_projection,
};
use solomon_sim::hud::quickbar::{
    NativeSkillQuickbarCooldownPresentation, layout_native_quickbar_binding,
    native_cooldown_sector_path, native_skill_quickbar_cooldown_presentation,
    native_skill_quickbar_icon_alpha,
};
use solomon_sim::player::PlayerState;
use solomon_sim::profile::{NATIVE_SKILL_CATALOG, native_skill_category};
use solomon_sim::secondary::{
    NativeSecondaryAbilityId, NativeSecondaryPlayerState,
    native_secondary_ability_mana_cost_with_stat_book,
};
use solomon_sim::settings::{GameControlBindings, game_binding_label};
use web_sys::HtmlElement;

use super::Hud;
use crate::ui::{append, element, style, text::paint_text_layout};

#[derive(Clone, PartialEq)]
pub(super) struct QuickbarSlot {
    pub label: String,
    pub binding_code: String,
    pub binding_label: String,
    pub entry_kind: &'static str,
    pub cooldown: NativeSkillQuickbarCooldownPresentation,
    pub mana_cost: Option<f64>,
    pub active: bool,
    pub combat_disabled: bool,
    pub insufficient_mana: bool,
    pub quantity: Option<u64>,
    pub icon_alpha: Option<f64>,
}

impl Hud {
    pub fn set_quickbar_player(
        &mut self,
        player: Option<&PlayerState>,
        secondary: Option<&NativeSecondaryPlayerState>,
        controls: &GameControlBindings,
        in_hub: bool,
    ) -> Result<(), String> {
        let mut slots = std::array::from_fn(|_| None);
        if let Some(player) = player {
            let factors = player.derived_stats().offensive_factors(&player.skill_runtime);
            for (slot, entry) in player
                .belt
                .entries
                .iter()
                .skip(self.hotbar.start())
                .take(NATIVE_BELT_SLOT_COUNT)
                .enumerate()
            {
                let binding_code = controls.belt[slot].clone();
                let binding_label = game_binding_label(&binding_code);
                let input = if binding_code.starts_with("Mouse") {
                    format!("{} button", binding_label.to_lowercase())
                } else {
                    format!("key {binding_label}")
                };
                let skill_id = match entry {
                    Some(NativeBeltEntry::Skill { skill_id }) => Some(*skill_id),
                    _ => None,
                };
                let category = skill_id.and_then(native_skill_category);
                let combat_disabled = in_hub && category == Some(2);
                let mana_cost = if category == Some(2) {
                    skill_id
                        .and_then(NativeSecondaryAbilityId::from_skill_id)
                        .map(|ability| {
                            native_secondary_ability_mana_cost_with_stat_book(
                                &player.book,
                                &player.stat_book,
                                ability,
                                &factors,
                            )
                        })
                        .or(Some(0.0))
                } else {
                    None
                };
                let insufficient_mana =
                    mana_cost.is_some_and(|cost| player.combat.current_mana < cost);
                let cooldown = if category == Some(2) {
                    let id = skill_id.expect("secondary skill ID");
                    native_skill_quickbar_cooldown_presentation(
                        secondary.map_or(0.0, |state| state.cooldown_ticks_by_skill[id]),
                        secondary.map_or(0.0, |state| state.cooldown_maximum_ticks_by_skill[id]),
                        secondary.map_or(0.0, |state| state.global_cooldown_ticks),
                    )
                } else {
                    NativeSkillQuickbarCooldownPresentation::default()
                };
                let active = skill_id.is_some_and(|id| {
                    id == player.book.primary_skill_id
                        || (category == Some(3)
                            && [
                                player.skill_runtime.concentration_skill_id_a,
                                player.skill_runtime.concentration_skill_id_b,
                            ]
                            .contains(&Some(id)))
                        || secondary.is_some_and(|state| match id {
                            12 => state.planewalker_ticks_remaining > 0,
                            23 => state.firewalker,
                            78 => state.mindstar,
                            79 => state.regenerate,
                            _ => false,
                        })
                });
                let potion = match entry {
                    Some(NativeBeltEntry::HealthPotion) => Some(native_belt_potion_projection(
                        &player.economy.backpack,
                        NativeBeltPotionKind::Health,
                    )),
                    Some(NativeBeltEntry::ManaPotion) => Some(native_belt_potion_projection(
                        &player.economy.backpack,
                        NativeBeltPotionKind::Mana,
                    )),
                    _ => None,
                };
                let item = entry.and_then(|entry| native_belt_entry_item(entry, &player.economy));
                let entry_kind = match entry {
                    None => "empty",
                    Some(NativeBeltEntry::Skill { .. }) => "skill",
                    Some(NativeBeltEntry::HealthPotion) => "health-potion",
                    Some(NativeBeltEntry::ManaPotion) => "mana-potion",
                    Some(NativeBeltEntry::Item { .. }) => "item",
                };
                let mut label = if entry.is_none() {
                    format!("Empty belt slot {}, {input}", slot + 1)
                } else {
                    let name = match entry {
                        Some(NativeBeltEntry::Skill { skill_id }) => {
                            NATIVE_SKILL_CATALOG[*skill_id].name
                        }
                        Some(NativeBeltEntry::HealthPotion) => "Health Potion",
                        Some(NativeBeltEntry::ManaPotion) => "Mana Potion",
                        _ => item.map_or("Unavailable item", |item| &item.name),
                    };
                    format!("{name}, {input}")
                };
                if let Some(potion) = &potion {
                    label.push_str(&format!(", {} available", potion.count));
                }
                if cooldown.remaining > 0.0 {
                    let seconds = js_sys::Number::from(cooldown.remaining / 100.0)
                        .to_fixed(2)
                        .map(String::from)
                        .map_err(|_| "Cooldown label formatting")?;
                    label.push_str(&format!(", {seconds} seconds cooldown remaining"));
                }
                if active {
                    label.push_str(", active");
                }
                if combat_disabled {
                    label.push_str(", unavailable in the Hub");
                }
                if insufficient_mana {
                    label.push_str(", insufficient mana");
                }
                slots[slot] = Some(QuickbarSlot {
                    label,
                    binding_code,
                    binding_label,
                    entry_kind,
                    cooldown,
                    mana_cost,
                    active,
                    combat_disabled,
                    insufficient_mana,
                    quantity: potion
                        .map(|potion| potion.count)
                        .or_else(|| item.map(|item| u64::from(item.quantity))),
                    icon_alpha: skill_id.map(|_| {
                        native_skill_quickbar_icon_alpha(
                            cooldown.remaining > 0.0,
                            combat_disabled || insufficient_mana,
                        )
                    }),
                });
            }
        }
        if self.quickbar != slots {
            self.quickbar = slots;
            self.previous = None;
        }
        Ok(())
    }

    pub(super) fn decorate_quickbar_slot(
        &self,
        button: &HtmlElement,
        sprite: &HtmlElement,
        slot: usize,
    ) -> Result<(), String> {
        let Some(row) = &self.quickbar[slot] else { return Ok(()) };
        for (key, value) in [
            ("data-binding-code", row.binding_code.as_str()),
            ("data-entry-kind", row.entry_kind),
            ("data-active", if row.active { "true" } else { "false" }),
        ] {
            button.set_attribute(key, value).map_err(|_| "Quickbar presentation state")?;
        }
        if let Some(cost) = row.mana_cost {
            button
                .set_attribute("data-mana-cost", &cost.to_string())
                .map_err(|_| "Quickbar mana cost")?;
        }
        if let Some(alpha) = row.icon_alpha {
            sprite
                .style()
                .set_property("opacity", &alpha.to_string())
                .map_err(|_| "Quickbar icon opacity")?;
            sprite
                .style()
                .set_property(
                    "filter",
                    if self.mode == super::HudMode::Hub { "brightness(0.25)" } else { "none" },
                )
                .map_err(|_| "Quickbar Hub icon tint")?;
            button
                .set_attribute("data-icon-alpha", &alpha.to_string())
                .map_err(|_| "Quickbar icon alpha")?;
        }
        if row.cooldown.remaining > 0.0 && row.cooldown.capacity > 0.0 {
            let namespace = Some("http://www.w3.org/2000/svg");
            let svg = self
                .document
                .create_element_ns(namespace, "svg")
                .map_err(|_| "Quickbar cooldown SVG")?;
            svg.set_attribute("class", "hud-belt-cooldown")
                .map_err(|_| "Quickbar cooldown class")?;
            svg.set_attribute("viewBox", "0 0 53 53").map_err(|_| "Quickbar cooldown square")?;
            svg.set_attribute("aria-hidden", "true").map_err(|_| "Quickbar cooldown decoration")?;
            let path = self
                .document
                .create_element_ns(namespace, "path")
                .map_err(|_| "Quickbar cooldown path")?;
            path.set_attribute(
                "d",
                &native_cooldown_sector_path(row.cooldown.remaining, row.cooldown.capacity),
            )
            .map_err(|_| "Quickbar cooldown sector")?;
            svg.append_child(&path).map_err(|_| "Quickbar cooldown path attachment")?;
            button.append_child(&svg).map_err(|_| "Quickbar cooldown attachment")?;
        }
        Ok(())
    }

    pub(super) fn draw_quickbar_binding(
        &self,
        button: &HtmlElement,
        slot: usize,
    ) -> Result<(), String> {
        let code = self.quickbar[slot].as_ref().map_or_else(
            || if slot == 0 { "Mouse2".to_owned() } else { format!("Digit{slot}") },
            |row| row.binding_code.clone(),
        );
        if code == "Mouse2" {
            return append(button, &self.ui.sprite(&self.document, 100, "hud-belt-mouse")?);
        }
        let label = self.quickbar[slot]
            .as_ref()
            .map_or_else(|| game_binding_label(&code), |row| row.binding_label.clone())
            .to_uppercase();
        let layout = layout_native_quickbar_binding(&label);
        let root = element(&self.document, "span", "hud-belt-binding")?;
        let backing = element(&self.document, "span", "hud-belt-key-backing")?;
        style(
            &backing,
            &format!("left:{}px;width:{}px", layout.backing_left, layout.backing_width),
        )?;
        for (left, width, source) in [
            (0.0, 5.0, 0.0),
            (5.0, (layout.backing_width - 10.0).max(0.0), -5.0),
            (layout.backing_width - 5.0, 5.0, -10.0),
        ] {
            let piece = element(&self.document, "span", "")?;
            style(
                &piece,
                &format!(
                    "left:{left}px;width:{width}px;background-image:url('assets/stock/hub-hud-key-backing.png');background-position:{source}px 0"
                ),
            )?;
            append(&backing, &piece)?;
        }
        append(&root, &backing)?;
        let text = element(&self.document, "span", "hud-belt-binding-glyphs")?;
        paint_text_layout(&self.document, &text, &layout.text)?;
        append(&root, &text)?;
        append(button, &root)
    }
}
