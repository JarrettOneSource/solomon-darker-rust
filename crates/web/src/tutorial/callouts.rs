//! Tutorial callouts over visible GPU control geometry, including compact scroll.
use super::{PointerPlan, TutorialPointerView, TutorialViewSnapshot};
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
        frame: &crate::gpu_ui::UiFrame<'_>,
        view: &TutorialViewSnapshot<'_>,
        viewport: HudRect,
    ) -> Self {
        let mut targets = Self::default();
        let anchor = |id: &str, height: f64| {
            frame.widget_rect(id).filter(|r| r.is_positive()).and_then(|r| {
                let center = r.center();
                (center.x as f64 >= viewport.x
                    && center.x as f64 <= viewport.x + viewport.width
                    && center.y as f64 >= viewport.y
                    && center.y as f64 <= viewport.y + viewport.height)
                    .then_some(Anchor {
                        rect: HudRect::new(
                            r.left() as f64,
                            r.top() as f64,
                            r.width() as f64,
                            r.height() as f64,
                        ),
                        scale: r.height() as f64 / height,
                    })
            })
        };
        match view.stage {
            5 => targets.secondary = anchor("hud:secondary-slot", 53.0),
            9 => targets.inventory = anchor("hud:inventory", 62.0),
            12 => targets.skills = anchor("hud:skills", 62.0),
            14 => {
                targets.primary = anchor("hud:skill-binding:12", 65.0);
                targets.concentration = anchor("hud:skill-binding:16", 65.0);
            }
            18 => {
                targets.health_potion = anchor("hud:health-potion", 50.0);
                targets.health_meter = anchor("hud:health", 20.0);
            }
            _ => {}
        }
        if matches!(view.stage, 10 | 13) {
            targets.resume = anchor("panel:close", 62.0);
            if view.stage == 10 {
                targets.belt_a = anchor("panel:belt:7", 53.0);
                targets.belt_b = anchor("panel:belt:6", 53.0);
                targets.equipment = anchor("panel:equipment:amulet", 46.0);
                if let Some(id) = view.amulet_item_id {
                    targets.amulet = anchor(&format!("panel:item:{id}"), 72.0);
                }
            } else {
                targets.belt_a = anchor("panel:belt:1", 53.0);
                targets.skill_hover = anchor("panel:skill-page:0:0", 87.0);
                targets.skill_concentration = anchor("panel:skill-page:2:0", 87.0);
            }
        }
        targets
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CalloutPlan {
    pub id: &'static str,
    pub text: String,
    pub center: Vec2,
    pub scale: f64,
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
