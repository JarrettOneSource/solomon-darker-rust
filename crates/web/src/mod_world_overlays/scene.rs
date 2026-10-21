//! Authored room SVG and host-only controls from `mod-ui/ModSceneOverlay.tsx`.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_protocol::hosted::ModAction;
use solomon_sim::modding::presentation::{mod_scene_color, mod_scene_number, project_mod_scene};
use solomon_sim::modding::{LuaObject, LuaValue};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element, Event, HtmlElement};

use crate::ui::{append, element};

pub struct ModViewAction {
    pub action: ModAction,
    pub target: String,
    pub arguments: LuaObject,
}

pub struct ModScenePartyMember<'a> {
    pub id: &'a str,
    pub display_name: &'a str,
}

type ClickListener = (HtmlElement, Closure<dyn FnMut(Event)>);

pub struct ModSceneOverlayFrame<'a> {
    pub runtime: Option<&'a LuaObject>,
    /// Source chooses run.runId, otherwise the actual local player ID.
    pub owner_id: &'a str,
    pub is_host: bool,
    pub party: &'a [ModScenePartyMember<'a>],
}

struct SceneActionState {
    content_id: String,
    room_index: usize,
    room_count: usize,
    is_host: bool,
}

pub struct ModSceneOverlay {
    document: Document,
    parent: HtmlElement,
    root: HtmlElement,
    title: HtmlElement,
    description: HtmlElement,
    room_count: HtmlElement,
    map: Element,
    previous: HtmlElement,
    next: HtmlElement,
    leave: HtmlElement,
    waiting: HtmlElement,
    current: Rc<RefCell<Option<SceneActionState>>>,
    actions: Rc<RefCell<Vec<ModViewAction>>>,
    listeners: Vec<ClickListener>,
}

impl ModSceneOverlay {
    pub(super) fn new(document: &Document, parent: &HtmlElement) -> Result<Self, String> {
        let root = element(document, "section", "mod-scene-overlay")?;
        let header = element(document, "header", "")?;
        let heading = element(document, "div", "")?;
        let title = element(document, "h2", "")?;
        let description = element(document, "p", "")?;
        let room_count = element(document, "span", "")?;
        append(&heading, &title)?;
        append(&header, &heading)?;
        append(&header, &room_count)?;
        append(&root, &header)?;
        let viewport = element(document, "div", "mod-scene-overlay__viewport")?;
        let map = svg_element(document, "svg")?;
        map.set_attribute("role", "img").map_err(|_| "Could not label the mod room map")?;
        viewport.append_child(&map).map_err(|_| "Could not attach the mod room map")?;
        append(&root, &viewport)?;
        let footer = element(document, "footer", "")?;
        let previous = element(document, "button", "")?;
        previous.set_text_content(Some("Previous room"));
        let next = element(document, "button", "")?;
        next.set_text_content(Some("Next room"));
        let leave = element(document, "button", "")?;
        leave.set_text_content(Some("Return to the Boneyard"));
        for button in [&previous, &next, &leave] {
            append(&footer, button)?;
        }
        append(&root, &footer)?;
        let waiting = element(document, "p", "mod-scene-overlay__waiting")?;
        waiting.set_text_content(Some("Waiting for the party leader…"));
        let current: Rc<RefCell<Option<SceneActionState>>> = Rc::new(RefCell::new(None));
        let actions = Rc::new(RefCell::new(Vec::new()));
        let mut listeners = Vec::new();
        for (button, direction) in [(&previous, -1_i32), (&next, 1), (&leave, 0)] {
            let state = current.clone();
            let outbox = actions.clone();
            let listener = Closure::<dyn FnMut(Event)>::new(move |_| {
                let state = state.borrow();
                let Some(state) = state.as_ref() else { return };
                if !state.is_host {
                    return;
                }
                let mut arguments = LuaObject::new();
                let action = if direction == 0 {
                    ModAction::SceneReturn
                } else {
                    let room = state.room_index as i64 + i64::from(direction);
                    if room < 0 || room as usize >= state.room_count {
                        return;
                    }
                    arguments.insert("room".into(), LuaValue::Number(room as f64));
                    ModAction::SceneRoom
                };
                outbox.borrow_mut().push(ModViewAction {
                    action,
                    target: state.content_id.clone(),
                    arguments,
                });
            });
            button
                .add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
                .map_err(|_| "Could not attach a mod room action")?;
            listeners.push((button.clone(), listener));
        }
        Ok(Self {
            document: document.clone(),
            parent: parent.clone(),
            root,
            title,
            description,
            room_count,
            map,
            previous,
            next,
            leave,
            waiting,
            current,
            actions,
            listeners,
        })
    }

    pub fn reset(&mut self) {
        self.root.remove();
        *self.current.borrow_mut() = None;
        self.actions.borrow_mut().clear();
    }

    pub fn take_actions(&mut self) -> Vec<ModViewAction> {
        std::mem::take(&mut self.actions.borrow_mut())
    }

    pub fn refresh(&mut self, frame: &ModSceneOverlayFrame<'_>) -> Result<(), String> {
        let Some(scene) = project_mod_scene(frame.runtime, frame.owner_id) else {
            self.reset();
            return Ok(());
        };
        let room = &scene.rooms[scene.room_index];
        super::attribute(&self.root, "aria-label", room.name)?;
        super::attribute(&self.root, "data-room-index", &scene.room_index.to_string())?;
        super::attribute(&self.root, "data-scene-content-id", scene.content_id)?;
        self.title.set_text_content(Some(room.name));
        self.description.set_text_content(Some(room.description));
        let heading = self.title.parent_element().ok_or("Mod room heading is detached")?;
        if room.description.is_empty() {
            self.description.remove();
        } else if self.description.parent_node().is_none() {
            heading
                .append_child(&self.description)
                .map_err(|_| "Could not attach mod room description")?;
        }
        self.room_count.set_text_content(Some(&format!(
            "Room {} / {}",
            scene.room_index + 1,
            scene.rooms.len()
        )));
        disabled(&self.previous, !frame.is_host || scene.room_index == 0)?;
        disabled(&self.next, !frame.is_host || scene.room_index + 1 >= scene.rooms.len())?;
        disabled(&self.leave, !frame.is_host)?;
        if frame.is_host {
            self.waiting.remove();
        } else if self.waiting.parent_node().is_none() {
            append(&self.root, &self.waiting)?;
        }
        *self.current.borrow_mut() = Some(SceneActionState {
            content_id: scene.content_id.to_owned(),
            room_index: scene.room_index,
            room_count: scene.rooms.len(),
            is_host: frame.is_host,
        });
        self.map
            .set_attribute("viewBox", &format!("0 0 {} {}", room.width, room.height))
            .map_err(|_| "Could not size mod room map")?;
        self.map
            .set_attribute("aria-label", &format!("{} map", room.name))
            .map_err(|_| "Could not label mod room map")?;
        self.map.set_text_content(None);
        let floor = svg_element(&self.document, "rect")?;
        svg_attributes(
            &floor,
            &[
                ("width", room.width.to_string()),
                ("height", room.height.to_string()),
                ("fill", room.floor.into()),
            ],
        )?;
        attach_svg(&self.map, &floor)?;
        let border = svg_element(&self.document, "rect")?;
        svg_attributes(
            &border,
            &[
                ("x", "4".into()),
                ("y", "4".into()),
                ("width", (room.width - 8.0).to_string()),
                ("height", (room.height - 8.0).to_string()),
                ("fill", "none".into()),
                ("stroke", "#8b7b9d".into()),
                ("stroke-width", "8".into()),
            ],
        )?;
        attach_svg(&self.map, &border)?;
        for wall in &room.walls {
            let rect = svg_element(&self.document, "rect")?;
            svg_attributes(
                &rect,
                &[
                    ("fill", mod_scene_color(wall.get("color"), "#51465f").into()),
                    ("height", mod_scene_number(wall.get("height"), 24.0).max(4.0).to_string()),
                    ("width", mod_scene_number(wall.get("width"), 80.0).max(4.0).to_string()),
                    ("x", mod_scene_number(wall.get("x"), 0.0).to_string()),
                    ("y", mod_scene_number(wall.get("y"), 0.0).to_string()),
                ],
            )?;
            attach_svg(&self.map, &rect)?;
        }
        for prop in &room.props {
            let group = svg_element(&self.document, "g")?;
            let x = mod_scene_number(prop.get("x"), room.width / 2.0);
            let y = mod_scene_number(prop.get("y"), room.height / 2.0);
            svg_attributes(&group, &[("transform", format!("translate({x} {y})"))])?;
            let circle = svg_element(&self.document, "circle")?;
            svg_attributes(
                &circle,
                &[
                    ("fill", mod_scene_color(prop.get("color"), "#a68bc6").into()),
                    ("r", mod_scene_number(prop.get("radius"), 18.0).max(8.0).to_string()),
                ],
            )?;
            attach_svg(&group, &circle)?;
            let label = prop
                .get("label")
                .and_then(LuaValue::as_str)
                .or_else(|| prop.get("kind").and_then(LuaValue::as_str))
                .unwrap_or("prop");
            svg_label(&self.document, &group, "34", label)?;
            attach_svg(&self.map, &group)?;
        }
        for (index, player) in frame.party.iter().enumerate() {
            let group = svg_element(&self.document, "g")?;
            let x =
                room.width / 2.0 + (index as f64 - (frame.party.len() as f64 - 1.0) / 2.0) * 44.0;
            let y = room.height * 0.72;
            svg_attributes(&group, &[("transform", format!("translate({x} {y})"))])?;
            let circle = svg_element(&self.document, "circle")?;
            svg_attributes(
                &circle,
                &[("class", "mod-scene-overlay__wizard".into()), ("r", "16".into())],
            )?;
            attach_svg(&group, &circle)?;
            svg_label(&self.document, &group, "32", player.display_name)?;
            attach_svg(&self.map, &group)?;
        }
        if self.root.parent_node().is_none() {
            append(&self.parent, &self.root)?;
        }
        Ok(())
    }
}

impl Drop for ModSceneOverlay {
    fn drop(&mut self) {
        self.root.remove();
        for (button, listener) in &self.listeners {
            // These valid callback identities can be removed even after the node detaches.
            let _ = button
                .remove_event_listener_with_callback("click", listener.as_ref().unchecked_ref());
        }
    }
}

fn disabled(button: &HtmlElement, disabled: bool) -> Result<(), String> {
    if disabled {
        button.set_attribute("disabled", "")
    } else {
        button.remove_attribute("disabled")
    }
    .map_err(|_| "Could not set mod room action state".into())
}

fn svg_element(document: &Document, tag: &str) -> Result<Element, String> {
    document
        .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
        .map_err(|_| format!("Could not create mod map {tag}"))
}

fn svg_attributes(node: &Element, attributes: &[(&str, String)]) -> Result<(), String> {
    for (name, value) in attributes {
        node.set_attribute(name, value).map_err(|_| format!("Could not set mod map {name}"))?;
    }
    Ok(())
}

fn attach_svg(parent: &Element, child: &Element) -> Result<(), String> {
    parent.append_child(child).map(|_| ()).map_err(|_| "Could not attach mod map geometry".into())
}

fn svg_label(document: &Document, parent: &Element, y: &str, text: &str) -> Result<(), String> {
    let label = svg_element(document, "text")?;
    label.set_attribute("y", y).map_err(|_| "Could not place mod map label")?;
    label.set_text_content(Some(text));
    attach_svg(parent, &label)
}
