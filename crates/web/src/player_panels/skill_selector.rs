//! The small HUD picker from `HudSkillSelector.tsx` and its native renderer.

use super::chrome::{art, button, place, text};
use super::{PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels};
use crate::ui::{append, element, style};
use solomon_sim::audio::AudioCue;
use solomon_sim::hud::HudRect;
use solomon_sim::hud::skill_selector::{
    NativeHudSkillSelectorTarget, native_hud_skill_selector_layout,
    native_hud_skill_selector_options,
};

impl PlayerPanels {
    pub fn show_skill_selector(&mut self, binding: usize, now: f64) -> Result<(), String> {
        let target = NativeHudSkillSelectorTarget::from_binding(binding)
            .ok_or("HUD skill selector requires binding 12, 16 or 20")?;
        self.show(PlayerPanel::SkillSelector(target), now)
    }

    pub(super) fn draw_hud_skill_selector(
        &self,
        target: NativeHudSkillSelectorTarget,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let options =
            native_hud_skill_selector_options(snapshot.book, snapshot.concentrations, target);
        button(
            &self.document,
            &self.content,
            "close",
            "Close skill selector",
            HudRect::new(0.0, 0.0, 1600.0, 900.0),
        )?;
        let title = text(
            &self.document,
            &self.content,
            target.title(),
            800.0,
            69.0,
            "panel-hud-selector-title",
            "#d9ba70",
        )?;
        let title_width = title.get_bounding_client_rect().width() / self.view_scale;
        let layout = native_hud_skill_selector_layout(options.len(), title_width);
        let mut rect = layout.panel;
        if self.compact {
            rect.width = rect.width.min(f64::from(self.viewport_size.0) - 16.0);
            rect.x = (f64::from(self.viewport_size.0) - rect.width) / 2.0;
        }
        let panel = element(&self.document, "div", "panel-hud-selector")?;
        place(&panel, rect)?;
        append(&self.content, &panel)?;
        append(&panel, &title)?;
        for (property, value) in [
            ("left", format!("{}px", rect.width / 2.0)),
            ("top", "17px".to_string()),
            ("opacity", ".75".to_string()),
        ] {
            title.style().set_property(property, &value).map_err(|_| "selector title placement")?;
        }
        let strip = element(&self.document, "div", "panel-hud-selector-options")?;
        // Keep a short row centered; a wider row starts at the native scroll origin.
        let strip_left = ((rect.width - layout.strip_width) / 2.0).max(0.0);
        style(
            &strip,
            &format!("left:{strip_left}px;top:22px;width:{}px", layout.strip_width.min(rect.width)),
        )?;
        append(&panel, &strip)?;
        for (index, option) in options.iter().enumerate() {
            let cell = element(&self.document, "div", "panel-hud-selector-cell")?;
            art(
                &self.document,
                &cell,
                &self.skills,
                option.icon_record,
                [26.0, 26.0],
                [1.0, 1.0],
                0.0,
            )?;
            let unavailable = target.concentration_slot().is_some() && snapshot.mind_chug_active;
            let label = if unavailable {
                format!("{}, unavailable while Mind Chug is active", option.name)
            } else {
                option.name.to_string()
            };
            let action = button(
                &self.document,
                &cell,
                &format!("hud-select-skill:{}", option.skill_id),
                &label,
                HudRect::new(0.0, 0.0, 52.0, 52.0),
            )?;
            if index == 0 {
                action
                    .set_attribute("data-game-default-focus", "true")
                    .map_err(|_| "selector initial focus")?;
            }
            let selected = target
                .concentration_slot()
                .map_or(snapshot.book.primary_skill_id == option.skill_id, |slot| {
                    snapshot.concentrations[slot] == Some(option.skill_id)
                });
            action
                .set_attribute("aria-pressed", if selected { "true" } else { "false" })
                .map_err(|_| "selector selected state")?;
            append(&strip, &cell)?;
        }
        Ok(())
    }

    pub(super) fn select_hud_skill(
        &mut self,
        skill_id: usize,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
    ) {
        let Some(PlayerPanel::SkillSelector(target)) = self.panel else {
            return;
        };
        if !native_hud_skill_selector_options(snapshot.book, snapshot.concentrations, target)
            .iter()
            .any(|option| option.skill_id == skill_id)
        {
            return;
        }
        if let Some(slot) = target.concentration_slot() {
            if !snapshot.mind_chug_active {
                self.actions.push(PlayerPanelAction::SelectConcentrationSlot { skill_id, slot });
                self.sound(AudioCue::Click);
                self.sound(AudioCue::Concentrate);
            }
        } else {
            self.actions.push(PlayerPanelAction::SelectPrimary(skill_id));
            self.sound(AudioCue::Click);
        }
        self.close(now, None);
    }
}
