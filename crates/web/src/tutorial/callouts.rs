//! Native Tutorial offsets over the live Inventory/Skillbook controls. Measuring the
//! existing DOM keeps the authored target correct during reveal, scrolling and rotation.

use wasm_bindgen::JsCast;
use web_sys::{Document, Element};

use super::{PointerPlan, TutorialPointerView, TutorialUi, TutorialViewSnapshot};
use crate::ui::{append, element, style};
use solomon_sim::boneyard::tutorial::tutorial_callout_geometry;
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use solomon_sim::{Vec2, hud::HudRect};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Anchor {
    rect: HudRect,
    pub scale: f64,
}

impl Anchor {
    pub fn center(self) -> Vec2 {
        Vec2::new(self.rect.x + self.rect.width / 2.0, self.rect.y + self.rect.height / 2.0)
    }
}

#[derive(Default)]
pub(super) struct TutorialTargets {
    pub secondary: Option<Anchor>,
    pub inventory: Option<Anchor>,
    pub skills: Option<Anchor>,
    pub primary: Option<Anchor>,
    pub concentration: Option<Anchor>,
    pub health_potion: Option<Anchor>,
    pub health_meter: Option<Anchor>,
    resume: Option<Anchor>,
    belt_a: Option<Anchor>,
    belt_b: Option<Anchor>,
    equipment: Option<Anchor>,
    amulet: Option<Anchor>,
    skill_hover: Option<Anchor>,
    skill_concentration: Option<Anchor>,
}

impl TutorialTargets {
    pub fn measure(
        document: &Document,
        view: &TutorialViewSnapshot<'_>,
        viewport: HudRect,
    ) -> Result<Self, String> {
        let mut targets = Self::default();
        let hud = |name: &str, height: f64| {
            anchor(
                document,
                &format!(".game-hud [data-tutorial-anchor='{name}']"),
                height,
                viewport,
            )
        };
        match view.stage {
            5 => targets.secondary = hud("secondary-slot", 53.0)?,
            9 => targets.inventory = hud("inventory", 62.0)?,
            12 => targets.skills = hud("skills", 62.0)?,
            14 => {
                targets.primary = hud("primary-skill", 65.0)?;
                targets.concentration = hud("concentration-a", 65.0)?;
            }
            18 => {
                targets.health_potion = hud("health-potion", 50.0)?;
                targets.health_meter = hud("health-meter", 20.0)?;
            }
            _ => {}
        }
        if !matches!(view.stage, 10 | 13) {
            return Ok(targets);
        }
        let panel = if view.stage == 10 { "inventory" } else { "skills" };
        let prefix = format!(".player-panels:not([hidden]) [data-player-panel='{panel}']");
        let modal = |action: &str, height: f64| {
            anchor(document, &format!("{prefix} [data-panel-action='{action}']"), height, viewport)
        };
        targets.resume = modal("close", 62.0)?;
        if targets.resume.is_none() {
            targets.resume = anchor(
                document,
                ".player-panels:not([hidden]) .player-panel-navigation [data-panel-action='close']",
                62.0,
                viewport,
            )?;
        }
        if view.stage == 10 {
            targets.belt_a = modal("belt:7", 53.0)?;
            targets.belt_b = modal("belt:6", 53.0)?;
            targets.equipment = modal("equipment:amulet", 46.0)?;
            if let Some(id) = view.amulet_item_id {
                targets.amulet = modal(&format!("item:{id}"), 72.0)?;
            }
        } else {
            targets.belt_a = modal("belt:1", 53.0)?;
            targets.skill_hover = anchor(
                document,
                &format!("{prefix} [data-tutorial-skill-page='0'][data-tutorial-skill-index='0']"),
                87.0,
                viewport,
            )?;
            targets.skill_concentration = anchor(
                document,
                &format!("{prefix} [data-tutorial-skill-page='2'][data-tutorial-skill-index='0']"),
                87.0,
                viewport,
            )?;
        }
        Ok(targets)
    }
}

fn anchor(
    document: &Document,
    selector: &str,
    native_height: f64,
    viewport: HudRect,
) -> Result<Option<Anchor>, String> {
    let nodes = document.query_selector_all(selector).map_err(|_| "Tutorial anchor selector")?;
    for index in 0..nodes.length() {
        let Some(node) = nodes.item(index) else {
            continue;
        };
        let node: Element = node.dyn_into().map_err(|_| "Tutorial anchor is not an element")?;
        if node.closest("[hidden]").map_err(|_| "Tutorial hidden anchor")?.is_some() {
            continue;
        }
        let computed = web_sys::window()
            .ok_or("Tutorial window")?
            .get_computed_style(&node)
            .map_err(|_| "Tutorial anchor style")?
            .ok_or("Tutorial anchor has no style")?;
        if computed.get_property_value("visibility").map_err(|_| "Tutorial anchor visibility")?
            == "hidden"
        {
            continue;
        }
        let rect = node.get_bounding_client_rect();
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        let target = Anchor {
            rect: HudRect::new(rect.left(), rect.top(), rect.width(), rect.height()),
            scale: rect.height() / native_height,
        };
        let center = target.center();
        let mut clip = viewport;
        if let Some(owner) =
            node.closest(".player-panel-viewport").map_err(|_| "Tutorial panel viewport")?
        {
            let rect = owner.get_bounding_client_rect();
            clip = HudRect::new(rect.left(), rect.top(), rect.width(), rect.height());
        }
        // Offscreen targets stay absent until the real scroll surface exposes them.
        if center.x < clip.x
            || center.x > clip.x + clip.width
            || center.y < clip.y
            || center.y > clip.y + clip.height
        {
            continue;
        }
        return Ok(Some(target));
    }
    Ok(None)
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CalloutPlan {
    id: &'static str,
    text: String,
    center: Vec2,
    scale: f64,
}

fn callout(
    plans: &mut Vec<CalloutPlan>,
    id: &'static str,
    text: &str,
    target: Anchor,
    dx: f64,
    dy: f64,
) {
    let center = target.center();
    plans.push(CalloutPlan {
        id,
        text: text.to_owned(),
        scale: target.scale,
        center: Vec2::new(center.x + dx * target.scale, center.y + dy * target.scale),
    });
}

fn pointer(
    plans: &mut Vec<PointerPlan>,
    id: &'static str,
    target: Anchor,
    dx: f64,
    dy: f64,
    blink: bool,
) {
    let center = target.center();
    plans.push(PointerPlan {
        id,
        blink,
        view: TutorialPointerView {
            origin: Vec2::new(center.x + dx * target.scale, center.y + dy * target.scale),
            target: center,
            scale: target.scale,
        },
    });
}

pub(super) fn modal_plans(
    view: &TutorialViewSnapshot<'_>,
    targets: &TutorialTargets,
    coarse: bool,
) -> (Vec<CalloutPlan>, Vec<PointerPlan>) {
    let mut callouts = Vec::new();
    let mut pointers = Vec::new();
    if !matches!(view.stage, 10 | 13) {
        return (callouts, pointers);
    }
    let inventory = view.stage == 10;
    if let Some(target) = targets.resume {
        callout(
            &mut callouts,
            "resume",
            &format!(
                "Click here or press '{}'\nagain to resume playing",
                view.resume_binding_label
            ),
            target,
            if inventory { -50.0 } else { 50.0 },
            if inventory { -120.0 } else { -110.0 },
        );
        pointer(
            &mut pointers,
            "modal-resume",
            target,
            if inventory { -50.0 } else { 40.0 },
            if inventory { -50.0 } else { -40.0 },
            true,
        );
    }
    if let Some(target) = targets.belt_a {
        callout(
            &mut callouts,
            "quick-use",
            if inventory {
                "Put items here\nfor quick use"
            } else {
                "Drag skills here\nfor quick use"
            },
            target,
            0.0,
            if inventory { -115.0 } else { -125.0 },
        );
        // Inventory's arrow starts over slot 7 and points toward slot 6.
        let tip = if inventory { targets.belt_b.unwrap_or(target) } else { target };
        let center = target.center();
        pointers.push(PointerPlan {
            id: "modal-quick-use",
            blink: false,
            view: TutorialPointerView {
                origin: Vec2::new(center.x - 20.0 * target.scale, center.y - 50.0 * target.scale),
                target: tip.center(),
                scale: target.scale,
            },
        });
    }
    if inventory {
        if let Some(target) = targets.equipment {
            callout(
                &mut callouts,
                "equipment",
                "Put equippable items\nhere to wear them.",
                target,
                -250.0,
                50.0,
            );
            pointer(&mut pointers, "modal-equipment", target, -60.0, 40.0, false);
        }
        if let Some(target) = targets.amulet {
            callout(
                &mut callouts,
                "backpack",
                "Found items go in your backpack.  Click and\ndrag to move items, double-click to use them.",
                target,
                374.0,
                -43.0,
            );
            // The stock painted tip stops 5px above the actual amulet cell.
            pointer(&mut pointers, "modal-backpack", target, 0.0, -69.5, false);
        }
    } else {
        if let Some(target) = targets.skill_concentration {
            pointer(&mut pointers, "modal-concentration", target, 100.0, -20.0, false);
            callout(
                &mut callouts,
                "concentration",
                "You are CONCENTRATING on\nyour new skill automatically",
                target,
                50.0,
                -165.0,
            );
            callout(
                &mut callouts,
                "concentration-limit",
                "This confers a bonus, but is\nlimited to one skill at a time.",
                target,
                50.0,
                -100.0,
            );
        }
        if let Some(target) = targets.skill_hover {
            pointer(&mut pointers, "modal-hover", target, -100.0, -40.0, false);
            callout(
                &mut callouts,
                "hover",
                if coarse {
                    "Tap a skill icon for\nmore information."
                } else {
                    "Hover your mouse over a\nskill icon for more information."
                },
                target,
                -115.0,
                -40.0,
            );
        }
    }
    (callouts, pointers)
}

impl TutorialUi {
    pub(super) fn draw_callouts(&mut self, plans: Vec<CalloutPlan>) -> Result<(), String> {
        if self.callout_plans == plans {
            return Ok(());
        }
        self.callouts.set_text_content(None);
        for plan in &plans {
            let scale = plan.scale;
            let geometry = tutorial_callout_geometry(
                &plan.text,
                plan.center.x / scale,
                plan.center.y / scale,
            )?;
            let node = element(&self.document, "div", "tutorial-callout")?;
            node.set_attribute("data-tutorial-callout", plan.id)
                .map_err(|_| "Tutorial callout id")?;
            style(
                &node,
                &format!(
                    "left:{}px;top:{}px;width:{}px;height:{}px;transform:scale({scale});transform-origin:0 0",
                    geometry.frame.x * scale,
                    geometry.frame.y * scale,
                    geometry.frame.width,
                    geometry.frame.height,
                ),
            )?;
            let frame = self.atlas.nine_slice(
                &self.document,
                4,
                geometry.frame.width,
                geometry.frame.height,
            )?;
            append(&node, &frame)?;
            for line in &geometry.lines {
                let spec = NativeUiTextSpec {
                    align: NativeUiTextAlign::Left,
                    tint: 0xd9ba70,
                    ..NativeUiTextSpec::new(
                        NativeUiFontName::Menu,
                        &line.text,
                        line.x - geometry.frame.x,
                        line.y - geometry.frame.y,
                    )
                };
                let text =
                    crate::ui::text::native_text(&self.document, &spec, "tutorial-callout-text")?;
                append(&node, &text)?;
            }
            append(&self.callouts, &node)?;
        }
        self.callout_plans = plans;
        Ok(())
    }
}
