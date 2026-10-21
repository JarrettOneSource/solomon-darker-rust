//! Stock Tutorial presentation from `TutorialOverlay.tsx`, `tutorial-hud-anchors.ts`
//! and `tutorial-modal-callouts.ts`. The session supplies canonical text and clocks;
//! this view only measures live controls and paints instructions over them.

mod callouts;

use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec};
use solomon_sim::{Vec2, hud::HudRect};
use web_sys::{Document, HtmlElement};

use crate::player_panels::chrome::{art, tint};
use crate::ui::{StockAtlas, append, element, style};

use callouts::{CalloutPlan, TutorialTargets, modal_plans};

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

pub struct TutorialUi {
    document: Document,
    root: HtmlElement,
    prelude: HtmlElement,
    prelude_skull: HtmlElement,
    prelude_record: HtmlElement,
    instructions: HtmlElement,
    callouts: HtmlElement,
    pointers: HtmlElement,
    narration: HtmlElement,
    atlas: StockAtlas,
    text_plans: Vec<TextPlan>,
    callout_plans: Vec<CalloutPlan>,
    pointer_plans: Vec<PointerPlan>,
    pointer_nodes: Vec<(HtmlElement, bool)>,
    previous_blink: Option<bool>,
    previous_intro: Option<(TutorialPreludeView, f64)>,
    narration_event_id: Option<u64>,
    viewport: (i32, i32),
}

impl TutorialUi {
    pub async fn attach(document: &Document) -> Result<Self, String> {
        let atlas = StockAtlas::load("UI").await?;
        let root = element(document, "div", "tutorial-ui")?;
        root.set_attribute("aria-live", "polite").map_err(|_| "Tutorial live instructions")?;
        root.set_hidden(true);
        let prelude = element(document, "div", "tutorial-prelude")?;
        prelude.set_attribute("role", "img").map_err(|_| "Tutorial prelude role")?;
        prelude
            .set_attribute("aria-label", "Midnight, six months ago")
            .map_err(|_| "Tutorial prelude label")?;
        let prelude_skull = art(document, &prelude, &atlas, 68, [0.0, 0.0], [4.0, 4.0], 0.0)?;
        prelude_skull.set_class_name("tutorial-prelude-skull");
        let record = art(document, &prelude, &atlas, 43, [0.0, 0.0], [1.0, 1.0], 0.0)?;
        record.set_class_name("tutorial-prelude-record");
        tint(document, &record, 0xd9ba70)?;
        let instructions = element(document, "div", "tutorial-instructions")?;
        let callouts = element(document, "div", "tutorial-callouts")?;
        let pointers = element(document, "div", "tutorial-pointers")?;
        let narration = element(document, "span", "tutorial-narration")?;
        narration.set_attribute("role", "status").map_err(|_| "Tutorial narration role")?;
        for node in [&prelude, &instructions, &callouts, &pointers, &narration] {
            append(&root, node)?;
        }
        append(&document.body().ok_or("Tutorial body")?, &root)?;
        Ok(Self {
            document: document.clone(),
            root,
            prelude,
            prelude_skull,
            prelude_record: record,
            instructions,
            callouts,
            pointers,
            narration,
            atlas,
            text_plans: Vec::new(),
            callout_plans: Vec::new(),
            pointer_plans: Vec::new(),
            pointer_nodes: Vec::new(),
            previous_blink: None,
            previous_intro: None,
            narration_event_id: None,
            viewport: (0, 0),
        })
    }

    pub fn update(
        &mut self,
        snapshot: Option<&TutorialViewSnapshot<'_>>,
        application_time_ms: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        let Some(view) = snapshot else {
            self.root.set_hidden(true);
            self.narration_event_id = None;
            return Ok(());
        };
        self.root.set_hidden(false);
        self.root
            .set_attribute("data-stage", &view.stage.to_string())
            .map_err(|_| "Tutorial stage")?;
        self.root
            .set_attribute("data-active", if view.active { "true" } else { "false" })
            .map_err(|_| "Tutorial active")?;
        if self.viewport != (width, height) {
            self.instructions.set_text_content(None);
            self.callouts.set_text_content(None);
            self.pointers.set_text_content(None);
            self.text_plans.clear();
            self.callout_plans.clear();
            self.pointer_plans.clear();
            self.pointer_nodes.clear();
            self.viewport = (width, height);
        }
        self.update_prelude(view.intro, view.text_scale)?;
        self.update_narration(view.narration)?;
        let viewport = HudRect::new(0.0, 0.0, f64::from(width), f64::from(height));
        let targets = TutorialTargets::measure(&self.document, view, viewport)?;
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
        self.draw_text(text)?;
        self.draw_callouts(callouts)?;
        self.draw_pointers(pointers)?;
        // App ticks continue while a modal pauses the simulation. Never use stage_ticks.
        let blink = (application_time_ms / 10.0).floor() as u64 % 50 > 19;
        if self.previous_blink != Some(blink) {
            for (node, blinking) in &self.pointer_nodes {
                node.style()
                    .set_property("opacity", if !blinking || blink { "1" } else { "0" })
                    .map_err(|_| "Tutorial pointer blink")?;
            }
            self.previous_blink = Some(blink);
        }
        Ok(())
    }

    fn update_prelude(
        &mut self,
        intro: Option<TutorialPreludeView>,
        scale: f64,
    ) -> Result<(), String> {
        self.prelude.set_hidden(intro.is_none());
        let key = intro.map(|intro| (intro, scale));
        if self.previous_intro == key {
            return Ok(());
        }
        self.previous_intro = key;
        if let Some(intro) = intro {
            style(&self.prelude, &format!("background-color:rgba(0,0,0,{})", intro.fade))?;
            // Preserve the atlas's logical size, registration and tint while animating.
            for (name, value) in [
                ("opacity", (intro.blend * intro.blend).to_string()),
                ("top", format!("calc(50% - {}px)", 100.0 * (1.0 + intro.blend) * scale)),
                ("transform", format!("translate(-50%,-50%) scale({})", 4.0 * scale)),
            ] {
                self.prelude_skull
                    .style()
                    .set_property(name, &value)
                    .map_err(|_| "Tutorial prelude skull motion")?;
            }
            self.prelude_record
                .style()
                .set_property("transform", &format!("translate(-50%,-50%) scale({scale})"))
                .map_err(|_| "Tutorial prelude record scale")?;
        }
        Ok(())
    }

    fn update_narration(
        &mut self,
        narration: Option<TutorialNarrationView<'_>>,
    ) -> Result<(), String> {
        let event_id = narration.map(|narration| narration.event_id);
        if self.narration_event_id == event_id {
            return Ok(());
        }
        self.narration_event_id = event_id;
        self.narration.set_text_content(
            narration
                .map(|narration| format!("{}: {}", narration.speaker, narration.text))
                .as_deref(),
        );
        Ok(())
    }

    fn draw_text(&mut self, plans: Vec<TextPlan>) -> Result<(), String> {
        if self.text_plans == plans {
            return Ok(());
        }
        self.instructions.set_text_content(None);
        if let Some(heading) = plans.iter().find(|plan| plan.id == "heading") {
            let mut text = heading.text.replace('\n', " ");
            if let Some(subheading) = plans.iter().find(|plan| plan.id == "subheading") {
                text.push_str(". ");
                text.push_str(&subheading.text);
            }
            let semantic = element(&self.document, "span", "tutorial-instruction-semantic")?;
            semantic.set_text_content(Some(&text));
            append(&self.instructions, &semantic)?;
        }
        for plan in &plans {
            // TutorialShadowedText paints a second bitmap pen at +2.25 native
            // pixels. Browser wrapping and font boxes do not alter its baseline.
            for (tint, offset, class) in [
                (0, 2.25 * plan.scale, "tutorial-instruction-text tutorial-instruction-shadow"),
                (0xd9ba70, 0.0, "tutorial-instruction-text"),
            ] {
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
                let node = crate::ui::text::native_text(&self.document, &spec, class)?;
                node.set_attribute("data-tutorial-text", plan.id)
                    .map_err(|_| "Tutorial text id")?;
                append(&self.instructions, &node)?;
            }
        }
        self.text_plans = plans;
        Ok(())
    }

    fn draw_pointers(&mut self, plans: Vec<PointerPlan>) -> Result<(), String> {
        if self.pointer_plans == plans {
            return Ok(());
        }
        self.pointers.set_text_content(None);
        self.pointer_nodes.clear();
        for plan in &plans {
            let view = plan.view;
            let rotation =
                (view.target.y - view.origin.y).atan2(view.target.x - view.origin.x).to_degrees()
                    + 90.0;
            let node = art(
                &self.document,
                &self.pointers,
                &self.atlas,
                28,
                [view.origin.x, view.origin.y],
                [view.scale, view.scale],
                rotation,
            )?;
            node.set_attribute("data-tutorial-pointer", plan.id)
                .map_err(|_| "Tutorial pointer id")?;
            for (name, value) in [
                ("x", view.origin.x),
                ("y", view.origin.y),
                ("target-x", view.target.x),
                ("target-y", view.target.y),
            ] {
                node.set_attribute(&format!("data-{name}"), &value.to_string())
                    .map_err(|_| "Tutorial pointer geometry")?;
            }
            self.pointer_nodes.push((node, plan.blink));
        }
        self.pointer_plans = plans;
        self.previous_blink = None;
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
