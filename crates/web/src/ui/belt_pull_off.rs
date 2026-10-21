//! Shared stock `NativeBeltPullOffBurst.tsx` art for gameplay and optional books.
//! Cosmetic random draws never enter an accepted simulation stream.

use super::{StockAtlas, append, element};
use solomon_sim::hud::belt_pull_off::{NativeBeltPullOffStep, native_belt_pull_off_burst_members};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element, Event, HtmlElement};

pub(crate) fn clear_belt_pull_off_bursts(root: &HtmlElement) -> Result<(), String> {
    let anchors =
        root.query_selector_all("[data-belt-burst-anchor]").map_err(|_| "belt burst owners")?;
    for index in 0..anchors.length() {
        if let Some(anchor) = anchors.item(index).and_then(|node| node.dyn_into::<Element>().ok()) {
            anchor.remove();
        }
    }
    Ok(())
}

pub(crate) fn belt_pull_off_burst(
    document: &Document,
    ui: &StockAtlas,
) -> Result<HtmlElement, String> {
    let burst = element(document, "span", "hud-belt-pull-off-burst")?;
    let step = if js_sys::Math::random() < 0.5 {
        NativeBeltPullOffStep::Degrees90
    } else {
        NativeBeltPullOffStep::Degrees120
    };
    let members = native_belt_pull_off_burst_members(step, &mut js_sys::Math::random);
    let mut terminal = 0;
    for index in 1..members.len() {
        if members[index].duration_ms > members[terminal].duration_ms {
            terminal = index;
        }
    }
    for (index, member) in members.iter().enumerate() {
        let sprite = ui.raw_sprite(document, member.record)?;
        sprite.set_class_name("hud-belt-burst-member");
        sprite
            .set_attribute("data-native-ui-record", &format!("UI.{}", member.record))
            .map_err(|_| "belt burst source record")?;
        if index == terminal {
            sprite
                .set_attribute("data-belt-burst-terminal", "true")
                .map_err(|_| "belt burst terminal member")?;
        }
        let geometry = ui.record(member.record)?;
        let style = sprite.style();
        for (name, value) in [
            ("margin-left", format!("{}px", -f64::from(geometry.width) / 2.0)),
            ("margin-top", format!("{}px", -f64::from(geometry.height) / 2.0)),
            ("--belt-burst-brightness", member.brightness.to_string()),
            ("--belt-burst-duration", format!("{}ms", member.duration_ms)),
            ("--belt-burst-end-x", format!("{}px", member.end.x)),
            ("--belt-burst-end-y", format!("{}px", member.end.y)),
            ("--belt-burst-rotation", format!("{}deg", member.rotation_degrees)),
            ("--belt-burst-scale-x", member.scale.x.to_string()),
            ("--belt-burst-scale-y", member.scale.y.to_string()),
            ("--belt-burst-start-x", format!("{}px", member.start.x)),
            ("--belt-burst-start-y", format!("{}px", member.start.y)),
        ] {
            style.set_property(name, &value).map_err(|_| "belt burst member style")?;
        }
        append(&burst, &sprite)?;
    }
    Ok(burst)
}

pub(crate) fn listen_to_burst_completion(
    root: &HtmlElement,
) -> Result<Closure<dyn FnMut(Event)>, String> {
    let listener = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
        let Some(target) = event.target().and_then(|target| target.dyn_into::<Element>().ok())
        else {
            return;
        };
        if !target.has_attribute("data-belt-burst-terminal") {
            return;
        }
        match target.closest("[data-belt-burst-anchor]") {
            Ok(Some(anchor)) => anchor.remove(),
            Ok(None) => {}
            Err(error) => web_sys::console::error_1(&error),
        }
    });
    root.add_event_listener_with_callback("animationend", listener.as_ref().unchecked_ref())
        .map_err(|_| "belt burst completion listener")?;
    Ok(listener)
}
