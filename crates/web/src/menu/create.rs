//! GPU Create art from Website `renderer/create-menu-renderer.ts`. The canonical
//! native motion, name admission and element VFX plans remain in `solomon-sim`.

use super::widgets::{self, Stage};
use crate::gpu_ui::{ImageShadow, StockAtlas, UiFrame};
use egui::{Align2, Color32, Pos2, Rect, Ui, Vec2};
use solomon_graphics::BlendMode;
use solomon_sim::element_vfx::{self, ElementVfxBlend, ElementVfxDraw};
use solomon_sim::menu::create_name::{finalize_create_wizard_name, validate_create_wizard_name};
use solomon_sim::menu::{self, CreateHandPose};
use solomon_sim::profile::create::NATIVE_CREATE_ENTRY_FADE_MS;
use solomon_sim::profile::{WizardDiscipline, WizardElement};

pub(super) struct CreateView {
    pub name: String,
    pub message: Option<String>,
    pub back_enabled: bool,
    pub focus_name: bool,
    create: StockAtlas,
    ui: StockAtlas,
    bad_guys: StockAtlas,
    selected: Option<WizardElement>,
    phase_elapsed: f64,
    scene_elapsed: f64,
    reduced_motion: bool,
    finalizing: bool,
}
impl CreateView {
    pub async fn new() -> Result<Self, String> {
        Ok(Self {
            name: "Helvidius".to_owned(),
            message: None,
            back_enabled: true,
            focus_name: false,
            create: StockAtlas::load("Create").await?,
            ui: StockAtlas::load("UI").await?,
            bad_guys: StockAtlas::load("BadGuys").await?,
            selected: None,
            phase_elapsed: 0.0,
            scene_elapsed: 0.0,
            reduced_motion: false,
            finalizing: false,
        })
    }
    pub fn restart(&mut self) {
        self.message =
            finalize_create_wizard_name(&self.name).err().map(|reason| reason.message().to_owned());
    }
    pub fn set_name(&mut self, value: &str) -> Result<(), String> {
        if let Some(reason) = validate_create_wizard_name(value)
            && !value.is_empty()
        {
            return Err(reason.message().to_owned());
        }
        self.name = value.to_owned();
        self.restart();
        Ok(())
    }
    pub fn finalized_name(&self) -> Result<String, String> {
        finalize_create_wizard_name(&self.name)
            .map(str::to_owned)
            .map_err(|reason| reason.message().to_owned())
    }
    pub fn update(
        &mut self,
        selected: Option<WizardElement>,
        phase_elapsed: f64,
        scene_elapsed: f64,
        reduced_motion: bool,
        finalizing: bool,
    ) -> Result<(), String> {
        self.selected = selected;
        self.phase_elapsed = phase_elapsed;
        self.scene_elapsed = scene_elapsed;
        self.reduced_motion = reduced_motion;
        self.finalizing = finalizing;
        Ok(())
    }
    pub fn default_focus(&self) -> Option<&'static str> {
        let elapsed = self.phase_elapsed.min(if self.selected.is_some() {
            menu::CREATE_SELECTION_ANIMATION_MS
        } else {
            menu::CREATE_ENTRY_ANIMATION_MS
        });
        let motion = if self.selected.is_some() {
            menu::create_selection_motion_at(elapsed)
        } else {
            menu::create_entry_motion_at(elapsed)
        };
        if self.selected.is_some() && motion.disciplines_visible {
            Some("discipline:arcane")
        } else if self.selected.is_none() && motion.elements_visible {
            Some("element:earth")
        } else {
            None
        }
    }
    pub fn paint(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        view: Rect,
        admitted: bool,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        widgets::gradient(ui, view, Color32::BLACK, widgets::color(0x404040));
        let stage_width = if view.width() < 650.0 { 900.0 } else { 1600.0 };
        let scale = (view.width() / stage_width).min(view.height() / 900.0);
        let stage = Stage {
            origin: Pos2::new(view.center().x - 800.0 * scale, view.bottom() - 900.0 * scale),
            scale,
        };
        let name_scale =
            if view.width() < 650.0 { ((view.width() - 24.0) / 484.0).min(1.0) } else { scale };
        let name_stage = Stage {
            origin: Pos2::new(
                view.center().x - 800.0 * name_scale,
                view.top() + if view.width() < 650.0 { 64.0 } else { 0.0 },
            ),
            scale: name_scale,
        };
        let dice = Stage { origin: Pos2::new(view.right() - 1600.0 * scale, view.top()), scale };
        self.create.paint_centered(
            frame,
            ui.painter(),
            7,
            stage.at(800.0, 800.0),
            scale * 3.0,
            if self.reduced_motion {
                0.0
            } else {
                (self.scene_elapsed / 1000.0 * 10.0).to_radians() as f32
            },
            widgets::alpha(0xffffff, 0.05),
        )?;
        if self.selected.is_some() && !self.reduced_motion {
            for index in 0..50 {
                let i = f64::from(index);
                let progress = (self.phase_elapsed - (i * 0.067 % 1.25) * 1000.0)
                    / ((2.6 + i * 0.19 % 2.1) * 1000.0);
                if !(0.0..=1.0).contains(&progress) {
                    continue;
                }
                let alpha = if progress < 0.12 {
                    0.78 * progress / 0.12
                } else if progress < 0.52 {
                    0.78 + (0.34 - 0.78) * (progress - 0.12) / 0.4
                } else {
                    0.34 * (1.0 - progress) / 0.48
                };
                let large = index % 5 == 0;
                let size = (if large { 23.2 / 59.0 } else { 3.52 / 3.0 })
                    * (0.55 + i * 0.23 % 0.9)
                    * (1.0 + progress * 0.1);
                self.create.paint(
                    frame,
                    ui.painter(),
                    if large { 4 } else { 8 },
                    stage.at(
                        (11.0 + i * 37.0) % 98.0 / 100.0 * 1600.0,
                        (5.0 + i * 29.0) % 92.0 / 100.0 * 900.0 - 117.0 + progress * 396.0,
                    ),
                    scale * size as f32,
                    widgets::alpha(0xffffff, alpha),
                )?;
            }
        }
        let elapsed = self.phase_elapsed.min(if self.selected.is_some() {
            menu::CREATE_SELECTION_ANIMATION_MS
        } else {
            menu::CREATE_ENTRY_ANIMATION_MS
        });
        let motion = if self.selected.is_some() {
            menu::create_selection_motion_at(elapsed)
        } else {
            menu::create_entry_motion_at(elapsed)
        };
        let idle = menu::create_hand_idle_offset_at(if self.reduced_motion {
            0.0
        } else {
            self.scene_elapsed
        });
        hand(
            frame,
            ui,
            stage,
            motion.left_pose,
            400.0 + motion.left_offset.x + motion.left_impulse.x + idle.x,
            560.0 + motion.left_offset.y + motion.left_impulse.y + idle.y,
            false,
        );
        let tick = if self.reduced_motion { 0.0 } else { (self.scene_elapsed / 10.0).floor() };
        let enabled = admitted
            && !self.finalizing
            && self.scene_elapsed >= NATIVE_CREATE_ENTRY_FADE_MS as f64;
        if self.selected.is_none() && motion.elements_visible {
            for (element, record) in [
                (WizardElement::Earth, 13),
                (WizardElement::Ether, 9),
                (WizardElement::Fire, 10),
                (WizardElement::Water, 12),
                (WizardElement::Air, 11),
            ] {
                let reveal = menu::create_element_reveal_motion_at(element, elapsed);
                let center = stage.at(reveal.position.x, reveal.position.y);
                let mut painter = ui.painter().clone();
                painter.multiply_opacity(reveal.opacity as f32);
                paint_vfx(
                    frame,
                    &painter,
                    &self.bad_guys,
                    center,
                    scale,
                    &element_vfx::native_element_vfx_plan(element, tick, 2.0),
                )?;
                let metrics = self.create.record(record)?;
                let size = Vec2::new(metrics.width as f32, metrics.height as f32) * scale;
                let rect = Rect::from_center_size(center, size.max(Vec2::splat(44.0)));
                let id = format!("element:{}", element.as_str());
                let response = widgets::interact_unstyled(
                    frame,
                    ui,
                    &id,
                    element.as_str(),
                    "button",
                    rect,
                    enabled,
                );
                let paint = frame.with_brightness(
                    if enabled && (response.hovered() || frame.focus_visible(&response)) {
                        1.3
                    } else {
                        1.0
                    },
                );
                self.create.paint_centered(
                    &paint,
                    ui.painter(),
                    record,
                    center,
                    scale,
                    0.0,
                    widgets::alpha(0xffffff, reveal.opacity),
                )?;
                widgets::focus_outline(frame, ui, &response, 2.0, 8.0, widgets::GOLD);
                if response.clicked() && enabled {
                    actions.push(id);
                }
            }
            self.create.paint(
                frame,
                ui.painter(),
                3,
                stage.at(620.0, 793.0),
                scale,
                Color32::WHITE,
            )?;
        }
        if let Some(element) = self.selected {
            let held = menu::create_selected_element_motion_at(element, elapsed);
            paint_vfx(
                frame,
                ui.painter(),
                &self.bad_guys,
                stage.at(held.position.x, held.position.y),
                scale,
                &element_vfx::native_element_vfx_plan(element, tick, held.scale * 2.0),
            )?;
        }
        hand(
            frame,
            ui,
            stage,
            motion.right_pose,
            1200.0 + motion.right_offset.x + motion.right_impulse.x + idle.x,
            560.0 + motion.right_offset.y + motion.right_impulse.y + idle.y,
            true,
        );
        if self.selected.is_some() && motion.disciplines_visible {
            for (discipline, record) in [
                (WizardDiscipline::Arcane, 0),
                (WizardDiscipline::Body, 1),
                (WizardDiscipline::Mind, 5),
            ] {
                let p = menu::create_discipline_reveal_motion_at(discipline, elapsed);
                let center = stage.at(p.x, p.y);
                let metrics = self.create.record(record)?;
                let size =
                    Vec2::new(metrics.logical_width as f32, metrics.logical_height as f32) * scale;
                let id = format!("discipline:{}", discipline.as_str());
                let response = widgets::interact_unstyled(
                    frame,
                    ui,
                    &id,
                    discipline.as_str(),
                    "button",
                    Rect::from_center_size(center, size.max(Vec2::splat(44.0))),
                    enabled,
                );
                let paint = frame.with_brightness(
                    if enabled && (response.hovered() || frame.focus_visible(&response)) {
                        1.3
                    } else {
                        1.0
                    },
                );
                self.create.paint_centered(
                    &paint,
                    ui.painter(),
                    record,
                    center,
                    scale,
                    0.0,
                    Color32::WHITE,
                )?;
                widgets::focus_outline(frame, ui, &response, 2.0, 8.0, widgets::GOLD);
                if response.clicked() && enabled {
                    actions.push(id);
                }
            }
            self.create.paint(
                frame,
                ui.painter(),
                2,
                stage.at(620.0, 793.0),
                scale,
                Color32::WHITE,
            )?;
        }
        self.paint_name(frame, ui, name_stage, admitted, actions)?;
        let back = Rect::from_min_size(view.min + Vec2::splat(4.0), Vec2::splat(44.0));
        let response = widgets::interact(
            frame,
            ui,
            "create-back",
            "Back",
            "button",
            back,
            enabled && self.back_enabled,
        );
        let paint = frame.with_brightness(if response.hovered() { 1.2 } else { 1.0 });
        self.ui.paint_centered(
            &paint,
            ui.painter(),
            42,
            back.center(),
            1.0,
            0.0,
            widgets::alpha(0xffffff, 0.76),
        )?;
        if response.clicked() && enabled && self.back_enabled {
            actions.push("create-back".to_owned());
        }
        let rect = dice.rect(1516.0, 0.0, 84.0, 60.0);
        let response = widgets::interact(
            frame,
            ui,
            "randomize-loadout",
            "Randomize wizard loadout",
            "button",
            rect,
            enabled && self.finalized_name().is_ok(),
        );
        let paint = frame.with_brightness(if response.hovered() { 1.2 } else { 1.0 });
        self.create.paint(&paint, ui.painter(), 6, dice.at(1518.0, 3.0), scale, Color32::WHITE)?;
        if response.clicked() && enabled {
            actions.push("randomize-loadout".to_owned());
        }
        Ok(())
    }
    fn paint_name(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        stage: Stage,
        enabled: bool,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let origin = stage.at(558.0, 17.0);
        let field = stage.rect(608.0, 29.0, 384.0, 49.0);
        widgets::gradient(ui, field, widgets::color(0x292929), widgets::color(0x202020));
        let metrics = self.ui.record(80)?;
        self.ui.paint_slice(
            frame,
            ui.painter(),
            80,
            Rect::from_min_size(Pos2::new(metrics.width as f32 - 1.0, 0.0), Vec2::new(1.0, 76.0)),
            stage.rect(630.0, 17.0, 340.0, 76.0),
            Color32::WHITE,
        )?;
        self.ui.paint(frame, ui.painter(), 80, origin, stage.scale, Color32::WHITE)?;
        let record_size =
            Vec2::new(metrics.logical_width as f32, metrics.logical_height as f32) * stage.scale;
        let right = Rect::from_min_size(
            stage.at(1042.0, 17.0) - Vec2::new(record_size.x, 0.0),
            record_size,
        );
        frame.image_quad(
            ui.painter(),
            self.ui.path(),
            [right.right_top(), right.left_top(), right.left_bottom(), right.right_bottom()],
            atlas_uv(frame, &self.ui, 80)?,
            Color32::WHITE,
        );
        widgets::image(
            frame,
            ui,
            "create-text-name-caption",
            stage.rect(732.0, 7.0, 136.0, 15.0),
            Color32::WHITE,
        );
        let response =
            crate::gpu_ui::NativeTextEdit::singleline(&mut self.name, "create-name", "Wizard name")
                .native_name(origin, stage.scale)
                .margin(Vec2::ZERO)
                .color(widgets::GOLD)
                .enabled(enabled)
                .readonly(self.finalizing)
                .rules(crate::gpu_ui::EditorRules {
                    validate: Some(validate_name_edit),
                    ..Default::default()
                })
                .request_focus(self.focus_name)
                .show(frame, ui, field);
        if ui.is_visible() && response.has_focus() {
            self.focus_name = false;
        }
        if let Some(reason) = response.rejected.as_ref() {
            self.message = Some(reason.clone());
            frame.announce("create-name:validation", reason);
        } else if response.changed() {
            self.restart();
        }
        widgets::focus_outline(frame, ui, &response, 2.0, 2.0, widgets::alpha(0xffffff, 0.8));
        let editable = enabled && !self.finalizing;
        let clear = stage.rect(958.0, 38.0, 30.0, 30.0);
        if widgets::interact(
            frame,
            ui,
            "clear-name",
            "Clear wizard name",
            "button",
            clear,
            editable,
        )
        .clicked()
            && editable
        {
            actions.push("clear-name".to_owned());
        }
        widgets::image(
            frame,
            ui,
            "create-text-name-caret",
            stage.rect(970.0, 47.0, 12.0, 11.0),
            widgets::alpha(0xffffff, 0.65),
        );
        let random = stage.rect(952.0, 92.0, 42.0, 30.0);
        let response = widgets::interact_unstyled(
            frame,
            ui,
            "randomize-name",
            "Randomize wizard name",
            "button",
            random,
            editable,
        );
        if response.clicked() && editable {
            actions.push("randomize-name".to_owned());
        }
        let hover = response.hovered();
        let paint = frame.with_brightness(if hover { 1.25 } else { 1.0 });
        self.create.paint_shadowed(
            &paint,
            ui.painter(),
            6,
            stage.at(952.0, 92.825),
            stage.scale * 0.525,
            ImageShadow {
                offset: Vec2::new(0.0, if hover { 0.0 } else { stage.scale }),
                blur: stage.scale * if hover { 3.0 } else { 1.0 },
                color: if hover { widgets::alpha(0xfff0b7, 0.55) } else { Color32::BLACK },
            },
            widgets::alpha(0xffffff, if hover { 1.0 } else { 0.78 }),
        )?;
        widgets::focus_outline(frame, ui, &response, 2.0, 2.0, widgets::alpha(0xffffff, 0.8));
        if let Some(error) = &self.message {
            let font =
                egui::FontId::new(14.0 * stage.scale, egui::FontFamily::Name("Georgia".into()));
            let size = ui.painter().layout_no_wrap(error.clone(), font, widgets::ERROR).size();
            frame.record_rect(
                "create-validation",
                error,
                "status",
                Rect::from_min_size(stage.at(800.0, 132.0) - Vec2::new(size.x / 2.0, 0.0), size),
                false,
            );
            frame.record_value("create-validation", error, None);
            widgets::label(
                ui,
                error,
                stage.at(800.0, 132.0),
                Align2::CENTER_TOP,
                "Georgia",
                14.0 * stage.scale,
                widgets::ERROR,
            );
        }
        Ok(())
    }
}
fn hand(
    frame: &UiFrame<'_>,
    ui: &Ui,
    stage: Stage,
    pose: CreateHandPose,
    x: f64,
    y: f64,
    flipped: bool,
) {
    let pose = match pose {
        CreateHandPose::Fist => "fist",
        CreateHandPose::Cupped => "cupped",
        CreateHandPose::Raised => "raised",
    };
    widgets::mirrored_image(
        frame,
        ui,
        &format!("create-hand-{pose}"),
        stage.rect(x - 315.0, y - 351.75, 630.0, 703.5),
        flipped,
        false,
        Color32::WHITE,
    );
}
fn atlas_uv(frame: &UiFrame<'_>, atlas: &StockAtlas, index: usize) -> Result<Rect, String> {
    let record = atlas.record(index)?;
    let size =
        frame.image_size(atlas.path()).ok_or_else(|| format!("{} is not ready", atlas.path()))?;
    Ok(Rect::from_min_max(
        Pos2::new(record.x as f32 / size.x, record.y as f32 / size.y),
        Pos2::new(
            ((record.x + record.width) as f32) / size.x,
            ((record.y + record.height) as f32) / size.y,
        ),
    ))
}
pub(super) fn paint_vfx(
    frame: &UiFrame<'_>,
    painter: &egui::Painter,
    atlas: &StockAtlas,
    origin: Pos2,
    outer_scale: f32,
    plan: &[ElementVfxDraw],
) -> Result<(), String> {
    for draw in plan {
        let records = draw.sprite.records();
        let index = records[draw.frame % records.len()];
        let record = atlas.record(index)?;
        let raw = [f64::from(record.width), f64::from(record.height)];
        let [w, h] = draw.sprite.registered_size().unwrap_or(raw);
        let [ax, ay] = draw.anchor.unwrap_or([0.5, 0.5]);
        let (left, top) = if draw.sprite.registered_size().is_some() {
            (
                (w - raw[0]) / 2.0 + f64::from(record.center_x),
                (h - raw[1]) / 2.0 + f64::from(record.center_y),
            )
        } else {
            (0.0, 0.0)
        };
        let pivot = origin + Vec2::new(draw.x as f32, draw.y as f32) * outer_scale;
        let scale = draw.scale as f32 * outer_scale;
        let angle = draw.rotation.to_radians() as f32;
        let rotate = |x: f64, y: f64| {
            let v = Vec2::new((x - ax * w) as f32, (y - ay * h) as f32) * scale;
            pivot + egui::emath::Rot2::from_angle(angle) * v
        };
        let quad = [
            rotate(left, top),
            rotate(left + raw[0], top),
            rotate(left + raw[0], top + raw[1]),
            rotate(left, top + raw[1]),
        ];
        let [r, g, b] = draw.color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        let tint = Color32::from_rgba_unmultiplied(
            r,
            g,
            b,
            (draw.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        );
        frame.image_quad_blended(
            painter,
            atlas.path(),
            quad,
            atlas_uv(frame, atlas, index)?,
            tint,
            if draw.blend == ElementVfxBlend::Add { BlendMode::Add } else { BlendMode::Normal },
        );
    }
    Ok(())
}

fn validate_name_edit(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        validate_create_wizard_name(value).map(|reason| reason.message().to_owned())
    }
}
