//! Stock Tutorial presentation. The accepted session supplies text and clocks;
//! every control anchor comes from the actual GPU widget registry.
mod callouts;
use crate::gpu_ui::{StockAtlas, UiFrame, native_text};
use callouts::{TutorialTargets, modal_plans};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec};
use solomon_sim::{Vec2, hud::HudRect};
use web_sys::Document;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TutorialPreludeView {
    pub blend: f64,
    pub fade: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TutorialPointerView {
    pub origin: Vec2,
    pub target: Vec2,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct TutorialNarrationView<'a> {
    pub event_id: u64,
    pub speaker: &'a str,
    pub text: &'a str,
}

/// Borrowed projection of the actual Tutorial. All positions and baselines are physical
/// client pixels; the session maps its camera/native frame exactly once before this call.
#[derive(Clone, Copy, Debug)]
pub struct TutorialViewSnapshot<'a> {
    pub stage: u8,
    pub active: bool,
    pub intro: Option<TutorialPreludeView>,
    pub heading: Option<&'a str>,
    pub subheading: Option<&'a str>,
    pub heading_baseline: Option<f64>,
    pub subheading_baseline: Option<f64>,
    pub text_scale: f64,
    pub coarse_pointer: bool,
    pub selected_skill_hud_acknowledged: bool,
    pub resume_binding_label: &'a str,
    pub amulet_item_id: Option<u64>,
    pub narration: Option<TutorialNarrationView<'a>>,
    pub world_target: Option<Vec2>,
    pub solomon_pointer: Option<TutorialPointerView>,
}

#[derive(Clone, Debug, PartialEq)]
struct TextPlan {
    id: &'static str,
    text: String,
    center: Vec2,
    heading: bool,
    scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct PointerPlan {
    id: &'static str,
    view: TutorialPointerView,
    blink: bool,
}

struct TutorialModel {
    stage: u8,
    active: bool,
    intro: Option<TutorialPreludeView>,
    heading: Option<String>,
    subheading: Option<String>,
    heading_baseline: Option<f64>,
    subheading_baseline: Option<f64>,
    text_scale: f64,
    coarse_pointer: bool,
    selected_skill_hud_acknowledged: bool,
    resume_binding_label: String,
    amulet_item_id: Option<u64>,
    narration: Option<(u64, String, String)>,
    world_target: Option<Vec2>,
    solomon_pointer: Option<TutorialPointerView>,
}
impl TutorialModel {
    fn from_view(v: &TutorialViewSnapshot<'_>) -> Self {
        Self {
            stage: v.stage,
            active: v.active,
            intro: v.intro,
            heading: v.heading.map(str::to_owned),
            subheading: v.subheading.map(str::to_owned),
            heading_baseline: v.heading_baseline,
            subheading_baseline: v.subheading_baseline,
            text_scale: v.text_scale,
            coarse_pointer: v.coarse_pointer,
            selected_skill_hud_acknowledged: v.selected_skill_hud_acknowledged,
            resume_binding_label: v.resume_binding_label.into(),
            amulet_item_id: v.amulet_item_id,
            narration: v.narration.map(|n| (n.event_id, n.speaker.into(), n.text.into())),
            world_target: v.world_target,
            solomon_pointer: v.solomon_pointer,
        }
    }
    fn view(&self) -> TutorialViewSnapshot<'_> {
        TutorialViewSnapshot {
            stage: self.stage,
            active: self.active,
            intro: self.intro,
            heading: self.heading.as_deref(),
            subheading: self.subheading.as_deref(),
            heading_baseline: self.heading_baseline,
            subheading_baseline: self.subheading_baseline,
            text_scale: self.text_scale,
            coarse_pointer: self.coarse_pointer,
            selected_skill_hud_acknowledged: self.selected_skill_hud_acknowledged,
            resume_binding_label: &self.resume_binding_label,
            amulet_item_id: self.amulet_item_id,
            narration: self.narration.as_ref().map(|(id, speaker, text)| TutorialNarrationView {
                event_id: *id,
                speaker,
                text,
            }),
            world_target: self.world_target,
            solomon_pointer: self.solomon_pointer,
        }
    }
}
pub struct TutorialUi {
    atlas: StockAtlas,
    model: Option<TutorialModel>,
    viewport: (i32, i32),
    now: f64,
}
impl TutorialUi {
    pub fn layer_id(&self) -> egui::LayerId {
        // Existing application body z40; all native Tutorial descendants stay here.
        egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("tutorial-overlay"))
    }

    pub async fn attach(_document: &Document) -> Result<Self, String> {
        Ok(Self { atlas: StockAtlas::load("UI").await?, model: None, viewport: (0, 0), now: 0.0 })
    }
    pub fn update(
        &mut self,
        snapshot: Option<&TutorialViewSnapshot<'_>>,
        application_time_ms: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        self.model = snapshot.map(TutorialModel::from_view);
        self.viewport = (width, height);
        self.now = application_time_ms;
        Ok(())
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some(model) = &self.model else { return Ok(()) };
        let view = &model.view();
        if view.intro.is_some() {
            frame.announce("Midnight, six months ago");
        }
        if let Some(heading) = view.heading {
            frame.announce(&format!(
                "{}. {}",
                heading.replace('\n', " "),
                view.subheading.unwrap_or("")
            ));
        }
        if let Some(narration) = view.narration {
            frame.announce(&format!("{}: {}", narration.speaker, narration.text));
        }
        let (width, height) = self.viewport;
        let viewport = HudRect::new(0.0, 0.0, width as f64, height as f64);
        let targets = TutorialTargets::measure(frame, view, viewport);
        let mut text = Vec::new();
        let mut pointers = Vec::new();
        let mut heading_baseline = view.heading_baseline;
        let mut subheading_baseline = view.subheading_baseline;
        // Stages 9/12 keep the native 95/125px clearance above the actual resized control.
        let instruction_target = match view.stage {
            9 => targets.inventory,
            12 => targets.skills,
            _ => None,
        };
        if let Some(anchor) = instruction_target {
            subheading_baseline = Some(anchor.center().y - 95.0 * anchor.scale);
            heading_baseline = Some(anchor.center().y - 125.0 * anchor.scale);
        }
        for (id, content, baseline, heading) in [
            ("heading", view.heading, heading_baseline, true),
            ("subheading", view.subheading, subheading_baseline, false),
        ] {
            if let (Some(content), Some(baseline)) = (content, baseline) {
                text.push(TextPlan {
                    id,
                    text: content.to_owned(),
                    center: Vec2::new(f64::from(width) / 2.0, baseline),
                    heading,
                    scale: view.text_scale,
                });
            }
        }
        match view.stage {
            5 => push_anchor_pointer(
                &mut pointers,
                "secondary-slot",
                targets.secondary,
                -70.0,
                -50.0,
            ),
            9 => push_anchor_pointer(&mut pointers, "inventory", targets.inventory, -40.0, -40.0),
            12 => push_anchor_pointer(&mut pointers, "skills", targets.skills, 40.0, -40.0),
            18 => {
                push_anchor_pointer(
                    &mut pointers,
                    "health-potion",
                    targets.health_potion,
                    -50.0,
                    -30.0,
                );
                push_anchor_pointer(
                    &mut pointers,
                    "health-meter",
                    targets.health_meter,
                    -100.0,
                    70.0,
                );
            }
            _ => {}
        }
        if view.stage == 14
            && !view.selected_skill_hud_acknowledged
            && let (Some(primary), Some(concentration)) = (targets.primary, targets.concentration)
        {
            let primary_center = primary.center();
            for (id, content, offset) in [
                ("selected-first", "click these icons to change your", 50.0),
                ("selected-second", "primary attack or concentration", 70.0),
            ] {
                text.push(TextPlan {
                    id,
                    text: content.to_owned(),
                    center: Vec2::new(
                        primary_center.x - 220.0 * primary.scale,
                        primary_center.y + offset * primary.scale,
                    ),
                    heading: false,
                    scale: view.text_scale,
                });
            }
            pointers.push(PointerPlan {
                id: "selected-skills",
                blink: true,
                view: TutorialPointerView {
                    origin: Vec2::new(
                        primary_center.x + 30.0 * primary.scale,
                        primary_center.y + 50.0 * primary.scale,
                    ),
                    target: Vec2::new(
                        (primary_center.x + concentration.center().x) / 2.0,
                        (primary_center.y + concentration.center().y) / 2.0,
                    ),
                    scale: primary.scale,
                },
            });
        }
        if view.intro.is_none()
            && let Some(pointer) = view.solomon_pointer
        {
            pointers.push(PointerPlan { id: "solomon-dig", view: pointer, blink: true });
        }
        if matches!(view.stage, 8 | 17)
            && let Some(target) = view.world_target
        {
            let target = Vec2::new(
                50.0_f64.max(target.x.min(f64::from(width) - 50.0)),
                50.0_f64.max(target.y.min(f64::from(height) - 50.0)),
            );
            pointers.push(PointerPlan {
                id: "world-sack",
                blink: true,
                view: TutorialPointerView {
                    origin: Vec2::new(target.x - 20.0, target.y - 60.0),
                    target,
                    scale: 1.0,
                },
            });
        }
        let (callouts, modal_pointers) = modal_plans(view, &targets, view.coarse_pointer);
        pointers.extend(modal_pointers);
        let painter = frame.ctx.layer_painter(self.layer_id());
        if let Some(intro) = view.intro {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width as f32, height as f32),
                ),
                0.0,
                egui::Color32::BLACK.gamma_multiply(intro.fade as f32),
            );
            self.atlas.paint_centered(
                frame,
                &painter,
                68,
                egui::pos2(
                    width as f32 / 2.0,
                    (height as f64 / 2.0 - 100.0 * (1.0 + intro.blend) * view.text_scale) as f32,
                ),
                (4.0 * view.text_scale) as f32,
                0.0,
                egui::Color32::WHITE.gamma_multiply((intro.blend * intro.blend) as f32),
            )?;
            self.atlas.paint_centered(
                frame,
                &painter,
                43,
                egui::pos2(width as f32 / 2.0, height as f32 / 2.0),
                view.text_scale as f32,
                0.0,
                egui::Color32::from_rgb(217, 186, 112),
            )?;
        }
        for plan in text {
            for (tint, offset) in [(0, 2.25 * plan.scale), (0xd9ba70, 0.0)] {
                let spec = NativeUiTextSpec {
                    scale: plan.scale,
                    tint,
                    ..NativeUiTextSpec::new(
                        if plan.heading {
                            NativeUiFontName::Heading
                        } else {
                            NativeUiFontName::Menu
                        },
                        &plan.text,
                        plan.center.x + offset,
                        plan.center.y + offset,
                    )
                };
                let layout = native_text(frame, &painter, &spec);
                if offset == 0.0 {
                    frame.record_rect(
                        &format!("tutorial:text:{}", plan.id),
                        &plan.text,
                        "text",
                        layout
                            .glyphs
                            .iter()
                            .map(|glyph| {
                                let ink =
                                    solomon_sim::native_ui_text::native_ui_glyph_ink_bounds(glyph);
                                egui::Rect::from_min_size(
                                    egui::pos2(ink.left as f32, ink.top as f32),
                                    egui::vec2(ink.width as f32, ink.height as f32),
                                )
                            })
                            .fold(egui::Rect::NOTHING, |a, b| a.union(b)),
                        false,
                    );
                }
            }
        }
        for plan in callouts {
            let scale = plan.scale;
            let geometry = solomon_sim::boneyard::tutorial::tutorial_callout_geometry(
                &plan.text,
                plan.center.x / scale,
                plan.center.y / scale,
            )?;
            let r = egui::Rect::from_min_size(
                egui::pos2((geometry.frame.x * scale) as f32, (geometry.frame.y * scale) as f32),
                egui::vec2(
                    (geometry.frame.width * scale) as f32,
                    (geometry.frame.height * scale) as f32,
                ),
            );
            self.atlas.paint_native_frame(frame, &painter, 4, r, true, egui::Color32::WHITE)?;
            for line in &geometry.lines {
                native_text(
                    frame,
                    &painter,
                    &NativeUiTextSpec {
                        align: solomon_sim::native_ui_text::NativeUiTextAlign::Left,
                        tint: 0xd9ba70,
                        scale,
                        ..NativeUiTextSpec::new(
                            NativeUiFontName::Menu,
                            &line.text,
                            line.x * scale,
                            line.y * scale,
                        )
                    },
                );
            }
            frame.record_rect(
                &format!("tutorial:callout:{}", plan.id),
                &plan.text,
                "text",
                r,
                false,
            );
        }
        let blink = (self.now / 10.0).floor() as u64 % 50 > 19;
        for plan in pointers {
            if plan.blink && !blink {
                continue;
            }
            let v = plan.view;
            let rotation = (v.target.y - v.origin.y).atan2(v.target.x - v.origin.x)
                + std::f64::consts::FRAC_PI_2;
            self.atlas.paint_centered(
                frame,
                &painter,
                28,
                egui::pos2(v.origin.x as f32, v.origin.y as f32),
                v.scale as f32,
                rotation as f32,
                egui::Color32::WHITE,
            )?;
            let record = self.atlas.record(28)?;
            frame.record_rect(
                &format!("tutorial:pointer:{}", plan.id),
                "Tutorial pointer",
                "image",
                egui::Rect::from_center_size(
                    egui::pos2(v.origin.x as f32, v.origin.y as f32),
                    egui::vec2(record.logical_width as f32, record.logical_height as f32)
                        * v.scale as f32,
                ),
                false,
            );
        }
        Ok(())
    }
}
fn push_anchor_pointer(
    plans: &mut Vec<PointerPlan>,
    id: &'static str,
    anchor: Option<callouts::Anchor>,
    offset_x: f64,
    offset_y: f64,
) {
    if let Some(anchor) = anchor {
        let target = anchor.center();
        plans.push(PointerPlan {
            id,
            blink: true,
            view: TutorialPointerView {
                origin: Vec2::new(
                    target.x + offset_x * anchor.scale,
                    target.y + offset_y * anchor.scale,
                ),
                target,
                scale: anchor.scale,
            },
        });
    }
}
