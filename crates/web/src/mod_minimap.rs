//! Source `mod-ui/ModMinimap.tsx`. The accepted model owns every marker and action;
//! the browser retains only keyed nodes and the last displayed action arguments.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use solomon_sim::Vec2;
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::projection::ModContentProjection;
use solomon_sim::modding::ui::{
    ModMinimapMarkerId, ModMinimapModel, ModMinimapSnapshot, project_mod_minimap,
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, HtmlElement};

use crate::ui::{append, element, javascript_string, style};

/// Root adds its request ID and sends the actual `ui-action` command with
/// `{action, arguments:{x,y}}`. A click keeps the center that was displayed then.
pub struct ModMinimapAction {
    pub content_id: String,
    pub action: String,
    pub center: Vec2,
}

struct ActionContext {
    content_id: String,
    center: Vec2,
}

pub struct ModMinimapView {
    document: Document,
    root: HtmlElement,
    markers: HtmlElement,
    bindings: HtmlElement,
    buttons: HtmlElement,
    marker_nodes: BTreeMap<String, HtmlElement>,
    binding_nodes: BTreeMap<String, HtmlElement>,
    action_labels: Vec<String>,
    current: Rc<RefCell<Option<ActionContext>>>,
    actions: Rc<RefCell<Vec<ModMinimapAction>>>,
    click: Closure<dyn FnMut(Event)>,
}

impl ModMinimapView {
    pub fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "section", "mod-minimap")?;
        root.set_hidden(true);
        let css = element(document, "style", "mod-minimap-styles")?;
        css.set_text_content(Some(include_str!("../../../web/mod-minimap.css")));
        append(&root, &css)?;
        let markers = element(document, "div", "")?;
        let bindings = element(document, "div", "")?;
        let buttons = element(document, "div", "")?;
        for group in [&markers, &bindings, &buttons] {
            append(&root, group)?;
        }
        let current: Rc<RefCell<Option<ActionContext>>> = Rc::new(RefCell::new(None));
        let actions = Rc::new(RefCell::new(Vec::new()));
        let context = current.clone();
        let outbox = actions.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(target) = event.target().and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            let target = target
                .closest("[data-mod-minimap-action]")
                .expect("the minimap action selector is valid");
            let Some(action) =
                target.and_then(|target| target.get_attribute("data-mod-minimap-action"))
            else {
                return;
            };
            if let Some(context) = context.borrow().as_ref() {
                outbox.borrow_mut().push(ModMinimapAction {
                    content_id: context.content_id.clone(),
                    action,
                    center: context.center,
                });
            }
        });
        root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "Could not listen to the authored minimap")?;
        append(&document.body().ok_or("The minimap needs a game document")?, &root)?;
        Ok(Self {
            document: document.clone(),
            root,
            markers,
            bindings,
            buttons,
            marker_nodes: BTreeMap::new(),
            binding_nodes: BTreeMap::new(),
            action_labels: Vec::new(),
            current,
            actions,
            click,
        })
    }

    /// Call on actual accepted snapshot/content/runtime updates, as Source's subscriptions do.
    pub fn update(
        &mut self,
        snapshot: &ModMinimapSnapshot<'_>,
        content: Option<&ModContentProjection>,
        viewer_id: &str,
        runtime: Option<&LuaObject>,
    ) -> Result<(), String> {
        let model =
            content.and_then(|content| project_mod_minimap(snapshot, content, viewer_id, runtime));
        let Some(model) = model else {
            self.hide();
            return Ok(());
        };
        self.paint(&model)
    }

    pub fn hide(&mut self) {
        self.root.set_hidden(true);
        *self.current.borrow_mut() = None;
        self.markers.set_text_content(None);
        self.bindings.set_text_content(None);
        self.buttons.set_text_content(None);
        self.marker_nodes.clear();
        self.binding_nodes.clear();
        self.action_labels.clear();
    }

    pub fn reset(&mut self) {
        self.hide();
        self.actions.borrow_mut().clear();
    }

    pub fn take_actions(&mut self) -> Vec<ModMinimapAction> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }

    fn paint(&mut self, model: &ModMinimapModel<'_>) -> Result<(), String> {
        self.root.set_hidden(false);
        attribute(&self.root, "aria-label", model.accessible_name)?;
        attribute(&self.root, "data-mount", model.mount)?;
        attribute(
            &self.root,
            "data-interactive",
            if model.actions.is_empty() { "false" } else { "true" },
        )?;
        style(&self.root, &format!("width:{}px;height:{}px", model.size.x, model.size.y))?;
        let mut retained = BTreeMap::new();
        for (index, marker) in model.markers.iter().enumerate() {
            let id = marker_id(marker.id);
            let key = format!("{}:{id}", marker.kind.as_str());
            let node = if let Some(node) = self.marker_nodes.remove(&key) {
                node
            } else {
                element(
                    &self.document,
                    "span",
                    &format!("mod-minimap__marker mod-minimap__marker--{}", marker.kind.as_str()),
                )?
            };
            let x = 50.0
                + (marker.position.x - model.center.x).clamp(-model.range, model.range)
                    / model.range
                    * 50.0;
            let y = 50.0
                + (marker.position.y - model.center.y).clamp(-model.range, model.range)
                    / model.range
                    * 50.0;
            style(&node, &format!("left:{x}%;top:{y}%"))?;
            attribute(&node, "data-mod-minimap-marker", &id)?;
            insert_at(&self.markers, &node, index)?;
            retained.insert(key, node);
        }
        for node in self.marker_nodes.values() {
            node.remove();
        }
        self.marker_nodes = retained;
        let mut retained = BTreeMap::new();
        if let Some(bindings) = model.bindings {
            for (index, (name, value)) in bindings.iter().enumerate() {
                let node = if let Some(node) = self.binding_nodes.remove(name) {
                    node
                } else {
                    element(&self.document, "output", "")?
                };
                let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
                let value = js_sys::JSON::parse(&json)
                    .map_err(|_| "Could not read an authored minimap binding")?;
                node.set_text_content(Some(&format!("{name}: {}", javascript_string(&value))));
                insert_at(&self.bindings, &node, index)?;
                retained.insert(name.to_owned(), node);
            }
        }
        for node in self.binding_nodes.values() {
            node.remove();
        }
        self.binding_nodes = retained;
        let action_labels: Vec<String> =
            model.actions.iter().map(|action| (*action).to_owned()).collect();
        if self.action_labels != action_labels {
            self.buttons.set_text_content(None);
            for action in &model.actions {
                let button = element(&self.document, "button", "")?;
                attribute(&button, "type", "button")?;
                attribute(&button, "data-mod-minimap-action", action)?;
                button.set_text_content(Some(action));
                append(&self.buttons, &button)?;
            }
            self.action_labels = action_labels;
        }
        *self.current.borrow_mut() =
            Some(ActionContext { content_id: model.content_id.to_owned(), center: model.center });
        Ok(())
    }
}

impl Drop for ModMinimapView {
    fn drop(&mut self) {
        self.root
            .remove_event_listener_with_callback("click", self.click.as_ref().unchecked_ref())
            .expect("remove the installed minimap listener");
        self.root.remove();
    }
}

fn marker_id(id: ModMinimapMarkerId<'_>) -> String {
    match id {
        ModMinimapMarkerId::Player(id) => id.to_owned(),
        ModMinimapMarkerId::Enemy(id) => {
            format!("enemy-{}", javascript_string(&JsValue::from_f64(id)))
        }
        ModMinimapMarkerId::ModEnemy(id) => {
            format!("mod-enemy-{}", javascript_string(&JsValue::from_f64(id)))
        }
        ModMinimapMarkerId::Powerup(id) => {
            format!("powerup-{}", javascript_string(&JsValue::from_f64(id)))
        }
    }
}

fn insert_at(parent: &HtmlElement, node: &HtmlElement, index: usize) -> Result<(), String> {
    let next = parent.children().item(index as u32);
    if next.as_ref() != Some(node.as_ref()) {
        parent
            .insert_before(node, next.as_ref().map(|node| node.as_ref()))
            .map_err(|_| "Could not retain authored minimap order")?;
    }
    Ok(())
}

fn attribute(node: &HtmlElement, name: &str, value: &str) -> Result<(), String> {
    node.set_attribute(name, value).map_err(|_| format!("Could not set minimap {name}"))
}
