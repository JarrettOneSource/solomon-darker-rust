//! Mounted `mod-ui/ModSkillQuickbar.tsx` with its stable keyboard controller.
//! The Game owner sends these cast intents through its existing request sequence.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use solomon_sim::modding::LuaObject;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::skill_quickbar::{
    ModCastAction, ModSkillQuickbarKey, ModSkillQuickbarPlayer, cast_mod_skill_quickbar_spell,
    mod_skill_quickbar_keyboard_slot, project_mod_skill_quickbar,
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, HtmlElement, KeyboardEvent, Window};

use crate::mod_presentation_assets::ModPresentationAssets;
use crate::ui::{append, element};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = Number)]
    fn javascript_number(value: &str) -> f64;
}

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
    button: HtmlElement,
    image: Option<HtmlElement>,
    label: HtmlElement,
}

pub struct ModSkillQuickbarView {
    document: Document,
    window: Window,
    root: HtmlElement,
    nav: HtmlElement,
    buttons: BTreeMap<i64, QuickbarButton>,
    order: Vec<i64>,
    current: Rc<RefCell<QuickbarState>>,
    actions: Rc<RefCell<Vec<ModCastAction>>>,
    click: Closure<dyn FnMut(Event)>,
    key_down: Closure<dyn FnMut(KeyboardEvent)>,
    session_id: Option<u64>,
    listening: bool,
}

impl ModSkillQuickbarView {
    pub fn new(window: &Window) -> Result<Self, String> {
        let document = window.document().ok_or("The quickbar needs a game document")?;
        let root = element(&document, "div", "mod-skill-quickbar-host")?;
        let css = element(&document, "style", "")?;
        css.set_text_content(Some(include_str!("../../../web/mod-skill-quickbar.css")));
        append(&root, &css)?;
        let nav = element(&document, "nav", "mod-skill-quickbar")?;
        nav.set_attribute("aria-label", "Mod spell quickbar")
            .map_err(|_| "Could not label the mod quickbar")?;
        let current = Rc::new(RefCell::new(QuickbarState::default()));
        let actions = Rc::new(RefCell::new(Vec::new()));

        let clicked_state = current.clone();
        let clicked_actions = actions.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(target) = event.target() else { return };
            let Ok(target) = target.dyn_into::<Element>() else { return };
            let Ok(Some(button)) = target.closest("[data-mod-quickbar-slot]") else { return };
            let Some(slot) = button.get_attribute("data-mod-quickbar-slot") else { return };
            let Ok(slot) = slot.parse::<i64>() else { return };
            let state = clicked_state.borrow();
            let Some(binding) = state.bindings.iter().find(|binding| binding.slot == slot) else {
                return;
            };
            if let Some(action) = cast_mod_skill_quickbar_spell(&binding.content_id, state.player) {
                clicked_actions.borrow_mut().push(action);
            }
        });
        nav.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "Could not attach the mod quickbar buttons")?;

        let keyboard_state = current.clone();
        let keyboard_actions = actions.clone();
        let key_down = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            let key = ModSkillQuickbarKey {
                number: javascript_number(&event.key()),
                shift: event.shift_key(),
                control: event.ctrl_key(),
                alt: event.alt_key(),
                meta: event.meta_key(),
                repeat: event.repeat(),
            };
            let Some(slot) = mod_skill_quickbar_keyboard_slot(key) else { return };
            let state = keyboard_state.borrow();
            let Some(binding) = state.bindings.iter().find(|binding| binding.slot == slot) else {
                return;
            };
            // The Source handler acts even in an input, or after another handler
            // prevented default. A bound row prevents default even without a player.
            event.prevent_default();
            if let Some(action) = cast_mod_skill_quickbar_spell(&binding.content_id, state.player) {
                keyboard_actions.borrow_mut().push(action);
            }
        });
        append(&document.body().ok_or("The quickbar needs a document body")?, &root)?;
        Ok(Self {
            document,
            window: window.clone(),
            root,
            nav,
            buttons: BTreeMap::new(),
            order: Vec::new(),
            current,
            actions,
            click,
            key_down,
            session_id: None,
            listening: false,
        })
    }

    /// Refresh accepted runtime and player inputs together. A new session ID forces
    /// the same listener detach/reattach as Source's changed session object.
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
        let slots = project_mod_skill_quickbar(runtime, assets);
        let live: BTreeSet<_> = slots.iter().map(|row| row.slot).collect();
        self.buttons.retain(|slot, nodes| {
            if live.contains(slot) {
                true
            } else {
                nodes.button.remove();
                false
            }
        });
        for slot in &slots {
            if !self.buttons.contains_key(&slot.slot) {
                let button = element(&self.document, "button", "")?;
                button.set_attribute("type", "button").map_err(|_| "Mod quickbar button type")?;
                button
                    .set_attribute("data-mod-quickbar-slot", &slot.slot.to_string())
                    .map_err(|_| "Mod quickbar button slot")?;
                let label = element(&self.document, "span", "")?;
                append(&button, &label)?;
                self.buttons.insert(slot.slot, QuickbarButton { button, image: None, label });
            }
            let nodes = self.buttons.get_mut(&slot.slot).expect("admitted quickbar button");
            nodes
                .button
                .set_attribute("title", &format!("{} (Shift+{})", slot.name, slot.slot + 1))
                .map_err(|_| "Mod quickbar title")?;
            nodes.label.set_text_content(Some(&(slot.slot + 1).to_string()));
            if let Some(asset) = slot.image {
                let image_url = images
                    .ok_or("Mod quickbar images have not been installed")?
                    .image(&asset.mod_id, &asset.path)?
                    .url();
                if nodes.image.is_none() {
                    let image = element(&self.document, "img", "")?;
                    image.set_attribute("alt", "").map_err(|_| "Mod quickbar image alt")?;
                    nodes
                        .button
                        .insert_before(&image, Some(&nodes.label))
                        .map_err(|_| "Mod quickbar image placement")?;
                    nodes.image = Some(image);
                }
                let image = nodes.image.as_ref().expect("admitted quickbar image");
                if image.get_attribute("src").as_deref() != Some(image_url) {
                    image.set_attribute("src", image_url).map_err(|_| "Mod quickbar image URL")?;
                }
            } else if let Some(image) = nodes.image.take() {
                image.remove();
            }
        }

        let next_order: Vec<_> = slots.iter().map(|row| row.slot).collect();
        if self.order != next_order {
            for slot in &next_order {
                append(&self.nav, &self.buttons[slot].button)?;
            }
            self.order = next_order;
        }
        if slots.is_empty() {
            self.nav.remove();
        } else if self.nav.parent_node().is_none() {
            append(&self.root, &self.nav)?;
        }
        let bindings = slots
            .iter()
            .map(|row| QuickbarBinding { slot: row.slot, content_id: row.content_id.to_owned() })
            .collect();
        *self.current.borrow_mut() = QuickbarState { bindings, player };
        self.set_listening(!slots.is_empty())
    }

    /// Source unmounts this component during College admission and session cleanup.
    pub fn reset(&mut self) -> Result<(), String> {
        self.set_listening(false)?;
        self.session_id = None;
        *self.current.borrow_mut() = QuickbarState::default();
        self.actions.borrow_mut().clear();
        self.buttons.clear();
        self.order.clear();
        self.nav.set_text_content(None);
        self.nav.remove();
        Ok(())
    }

    pub fn take_actions(&mut self) -> Vec<ModCastAction> {
        std::mem::take(&mut self.actions.borrow_mut())
    }

    fn set_listening(&mut self, listening: bool) -> Result<(), String> {
        if self.listening == listening {
            return Ok(());
        }
        let callback = self.key_down.as_ref().unchecked_ref();
        if listening {
            self.window
                .add_event_listener_with_callback("keydown", callback)
                .map_err(|_| "Could not attach the mod quickbar keyboard")?;
        } else {
            self.window
                .remove_event_listener_with_callback("keydown", callback)
                .map_err(|_| "Could not remove the mod quickbar keyboard")?;
        }
        self.listening = listening;
        Ok(())
    }
}

impl Drop for ModSkillQuickbarView {
    fn drop(&mut self) {
        let _ = self.set_listening(false);
        let _ = self
            .nav
            .remove_event_listener_with_callback("click", self.click.as_ref().unchecked_ref());
        self.root.remove();
    }
}
