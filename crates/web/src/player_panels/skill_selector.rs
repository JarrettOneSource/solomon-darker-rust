//! HUD primary/concentration selector, using its native horizontal strip.
use solomon_sim::audio::AudioCue;
use solomon_sim::hud::HudRect;
use solomon_sim::hud::skill_selector::{
    NativeHudSkillSelectorTarget, native_hud_skill_selector_layout,
    native_hud_skill_selector_options,
};

use super::chrome::PanelCanvas;
use super::{PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels};
impl PlayerPanels {
    pub fn show_skill_selector(&mut self, binding: usize, now: f64) -> Result<(), String> {
        let target = NativeHudSkillSelectorTarget::from_binding(binding)
            .ok_or("HUD skill selector requires binding 12, 16 or 20")?;
        self.show(PlayerPanel::SkillSelector(target), now)
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
    pub(super) fn draw_hud_skill_selector(
        &self,
        c: &PanelCanvas<'_, '_>,
        target: NativeHudSkillSelectorTarget,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let options = native_hud_skill_selector_options(s.book, s.concentrations, target);
        c.button("close", "Close skill selector", HudRect::new(0.0, 0.0, 1600.0, 900.0));
        let width =
            c.ui.painter()
                .layout_no_wrap(
                    target.title().into(),
                    egui::FontId::new(16.0, egui::FontFamily::Name("Palatino".into())),
                    egui::Color32::WHITE,
                )
                .size()
                .x as f64;
        let layout = native_hud_skill_selector_layout(options.len(), width);
        let mut panel = layout.panel;
        if self.compact {
            panel.width = panel.width.min(self.viewport_size.0 as f64 - 16.0);
            panel.x = (self.viewport_size.0 as f64 - panel.width) / 2.0;
        }
        c.fill(panel, 0, 0.95);
        c.browser_text(target.title(), panel.x + panel.width / 2.0, panel.y + 17.0, 16.0, 0xd9ba70);
        c.semantic("panel:skill-selector", target.title(), "dialog", panel);
        let left = ((panel.width - layout.strip_width) / 2.0).max(0.0);
        let region =
            HudRect::new(panel.x + left, panel.y + 22.0, layout.strip_width.min(panel.width), 52.0);
        let mut child = super::chrome::child_ui(c.ui, "hud-skill-options", c.rect(region));
        let mut result = Ok(());
        egui::ScrollArea::horizontal().id_salt("hud-skill-options").show(&mut child, |ui| {
            let origin = ui.cursor().min;
            let strip = c.on_ui(ui).at(origin.x as f64, origin.y as f64);
            for (index, option) in options.iter().enumerate() {
                let cell = strip.at(index as f64 * 52.0, 0.0);
                if let Err(message) = cell.art(
                    &self.skills,
                    option.icon_record,
                    [26.0, 26.0],
                    [1.0, 1.0],
                    0.0,
                    0xffffff,
                ) {
                    result = Err(message);
                }
                let unavailable = target.concentration_slot().is_some() && s.mind_chug_active;
                let label = if unavailable {
                    format!("{}, unavailable while Mind Chug is active", option.name)
                } else {
                    option.name.to_string()
                };
                let selected = target
                    .concentration_slot()
                    .map_or(s.book.primary_skill_id == option.skill_id, |slot| {
                        s.concentrations[slot] == Some(option.skill_id)
                    });
                cell.button(
                    &format!("hud-select-skill:{}", option.skill_id),
                    &label,
                    HudRect::new(0.0, 0.0, 52.0, 52.0),
                );
                c.frame.record_value(
                    &format!("panel:hud-select-skill:{}", option.skill_id),
                    &option.skill_id.to_string(),
                    Some(selected),
                );
                if selected {
                    cell.stroke(HudRect::new(0.0, 0.0, 52.0, 52.0), 0x97c797, 2.0);
                }
            }
            ui.set_min_size(egui::vec2(layout.strip_width as f32, 52.0));
        });
        result
    }
}
