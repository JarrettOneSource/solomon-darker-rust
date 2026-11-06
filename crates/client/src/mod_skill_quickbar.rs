//! Authored skill quickbar with keyboard bindings owned for the session lifetime.

use crate::gpu_ui::UiFrame;
use crate::mod_presentation_assets::ModPresentationAssets;
use crate::platform::{KeyboardEffect, KeyboardListener, Surface};
use egui::{Color32, FontFamily, FontId, Stroke, vec2};
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::skill_quickbar::{
    ModCastAction, ModSkillQuickbarKey, ModSkillQuickbarPlayer, cast_mod_skill_quickbar_spell,
    mod_skill_quickbar_keyboard_slot, project_mod_skill_quickbar,
};
use std::cell::RefCell;
use std::rc::Rc;

struct QuickbarBinding {
    slot: i64,
    content_id: String,
}

#[derive(Default)]
struct QuickbarState {
    bindings: Vec<QuickbarBinding>,
    player: Option<ModSkillQuickbarPlayer>,
}

struct QuickbarButton {
    slot: i64,
    content_id: String,
    name: String,
    image_url: Option<String>,
}
pub struct ModSkillQuickbarView {
    surface: Surface,
    buttons: Vec<QuickbarButton>,
    current: Rc<RefCell<QuickbarState>>,
    actions: Rc<RefCell<Vec<ModCastAction>>>,
    keyboard: Option<KeyboardListener>,
    session_id: Option<u64>,
}
impl ModSkillQuickbarView {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Middle, egui::Id::unique("mod/quickbar"))
    }

    pub fn new(surface: &Surface) -> Result<Self, String> {
        Ok(Self {
            surface: surface.clone(),
            buttons: Vec::new(),
            current: Rc::new(RefCell::new(QuickbarState::default())),
            actions: Rc::new(RefCell::new(Vec::new())),
            keyboard: None,
            session_id: None,
        })
    }
    pub fn update(
        &mut self,
        session_id: u64,
        runtime: Option<&LuaObject>,
        player: Option<ModSkillQuickbarPlayer>,
        assets: &[GameModAsset],
        images: Option<&ModPresentationAssets>,
    ) -> Result<(), String> {
        if self.session_id.is_some_and(|current| current != session_id) {
            self.set_listening(false)?;
            self.actions.borrow_mut().clear();
        }
        self.session_id = Some(session_id);
        self.buttons.clear();
        for row in project_mod_skill_quickbar(runtime, assets) {
            let image_url = if let Some(asset) = row.image {
                Some(
                    images
                        .ok_or("Mod quickbar images are not installed")?
                        .image(&asset.mod_id, &asset.path)?
                        .url()
                        .to_owned(),
                )
            } else {
                None
            };
            self.buttons.push(QuickbarButton {
                slot: row.slot,
                content_id: row.content_id.to_owned(),
                name: row.name.to_owned(),
                image_url,
            });
        }
        *self.current.borrow_mut() = QuickbarState {
            bindings: self
                .buttons
                .iter()
                .map(|row| QuickbarBinding { slot: row.slot, content_id: row.content_id.clone() })
                .collect(),
            player,
        };
        self.set_listening(!self.buttons.is_empty())
    }
    pub fn reset(&mut self) -> Result<(), String> {
        self.set_listening(false)?;
        self.session_id = None;
        *self.current.borrow_mut() = QuickbarState::default();
        self.actions.borrow_mut().clear();
        self.buttons.clear();
        Ok(())
    }
    pub fn take_actions(&mut self) -> Vec<ModCastAction> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        if self.buttons.is_empty() {
            return Ok(());
        }
        let ctx = frame.ctx.clone();
        egui::Area::new(self.layer_id().id)
            .fade_in(false)
            .order(self.layer_id().order)
            .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, -80.0))
            .show(&ctx, |ui| -> Result<(), String> {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.horizontal(|ui| -> Result<(), String> {
                    for row in &self.buttons {
                        let (bounds, response) =
                            ui.allocate_exact_size(vec2(41.6, 41.6), egui::Sense::click());
                        ui.painter().rect_filled(
                            bounds,
                            0.0,
                            Color32::from_rgba_unmultiplied(9, 10, 14, 230),
                        );
                        ui.painter().rect_stroke(
                            bounds,
                            0.0,
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(202, 181, 126, 204)),
                            egui::StrokeKind::Inside,
                        );
                        if let Some(url) = &row.image_url {
                            let content = bounds.shrink(4.2);
                            if let Some(size) = frame.image_size(url) {
                                let fit = size
                                    * (content.width() / size.x).min(content.height() / size.y);
                                let image = egui::Rect::from_center_size(content.center(), fit);
                                frame.image_sampled(
                                    ui.painter(),
                                    url,
                                    image,
                                    None,
                                    Color32::WHITE,
                                    crate::gpu_ui::TextureSampling::Nearest,
                                );
                                let id = format!("mod/quickbar-icon:{}", row.slot);
                                frame.record_rect(
                                    &id,
                                    &format!("{} icon", row.name),
                                    "img",
                                    image,
                                    true,
                                );
                                frame.record_value(
                                    &id,
                                    &format!("{}x{} {url}", size.x, size.y),
                                    None,
                                );
                            }
                        }
                        let slot = crate::gpu_ui::font_run(
                            frame.ctx,
                            FontId::new(9.28, FontFamily::Name("Alegreya Bold".into())),
                            Color32::from_rgb(241, 230, 198),
                            &(row.slot + 1).to_string(),
                            None,
                        )?;
                        frame.paint_group(
                            ui.painter(),
                            crate::gpu_ui::GroupEffect {
                                shadows: vec![crate::gpu_ui::ImageShadow {
                                    offset: vec2(0.0, 1.0),
                                    blur: 1.0,
                                    color: Color32::BLACK,
                                }],
                                ..Default::default()
                            },
                            |frame, painter| {
                                slot.paint(
                                    frame,
                                    painter,
                                    bounds.right_bottom() - vec2(2.92, 1.0) - slot.size(),
                                    1.0,
                                )
                            },
                        )?;
                        crate::menu::widgets::focus_outline(
                            frame,
                            ui,
                            &response,
                            2.0,
                            2.0,
                            Color32::from_rgba_unmultiplied(65, 227, 255, 179),
                        );
                        frame.record(
                            &format!("mod/quickbar:{}", row.slot),
                            &format!("{} (Shift+{})", row.name, row.slot + 1),
                            "button",
                            &response,
                        );
                        if response.clicked()
                            && let Some(action) = cast_mod_skill_quickbar_spell(
                                &row.content_id,
                                self.current.borrow().player,
                            )
                        {
                            self.actions.borrow_mut().push(action);
                        }
                    }
                    Ok(())
                })
                .inner?;
                Ok(())
            })
            .inner?;
        Ok(())
    }
    fn set_listening(&mut self, listening: bool) -> Result<(), String> {
        if self.keyboard.is_some() == listening {
            return Ok(());
        }
        self.keyboard = if listening {
            let current = self.current.clone();
            let actions = self.actions.clone();
            Some(KeyboardListener::attach(&self.surface, move |event| {
                let key = event.stroke;
                if !key.pressed {
                    return KeyboardEffect::Pass;
                }
                let key = ModSkillQuickbarKey {
                    number: crate::ui::parse_javascript_number(&key.key),
                    shift: key.shift,
                    control: key.ctrl,
                    alt: key.alt,
                    meta: key.meta,
                    repeat: key.repeat,
                };
                let Some(slot) = mod_skill_quickbar_keyboard_slot(key) else {
                    return KeyboardEffect::Pass;
                };
                let state = current.borrow();
                let Some(binding) = state.bindings.iter().find(|binding| binding.slot == slot)
                else {
                    return KeyboardEffect::Pass;
                };
                // Source also handles editable or previously prevented key events.
                if let Some(action) =
                    cast_mod_skill_quickbar_spell(&binding.content_id, state.player)
                {
                    actions.borrow_mut().push(action);
                }
                KeyboardEffect::PreventDefault
            })?)
        } else {
            None
        };
        Ok(())
    }
}
