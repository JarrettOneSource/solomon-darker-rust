//! Source `mod-ui/ModPanels.tsx`. The host owns spells, shop stock, prices and
//! portals; this view only projects its rows and queues the actual selected IDs.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::Vec2;
use solomon_sim::economy::{HubInventoryItem, HubItemKind};
use solomon_sim::hub::regions::HubRegion;
use solomon_sim::modding::{LuaObject, LuaValue};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, HtmlElement};

use crate::ui::{append, element, javascript_string};

#[derive(Clone, Copy)]
pub enum ModPanelsWorld {
    Hub { region: Option<HubRegion> },
    Boneyard,
}

pub struct ModPanelsPlayer<'a> {
    pub position: Vec2,
    pub backpack: &'a [HubInventoryItem],
}

pub struct ModPanelsSceneObject {
    pub type_id: u32,
    pub position: Vec2,
}

pub struct ModPanelsSnapshot<'a> {
    pub runtime: Option<&'a LuaObject>,
    pub player: Option<ModPanelsPlayer<'a>>,
    pub world: ModPanelsWorld,
    /// The actual `session.getBoneyard()?.scene.objects` projection, or empty.
    pub scene_objects: &'a [ModPanelsSceneObject],
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModPanelAction {
    Cast { content_id: String, position: Vec2 },
    ShopBuy { content_id: String, row: usize },
    Reforge { content_id: String, item_id: u64, service: usize },
    PortalEnter { portal_id: String },
}

struct ButtonRow {
    key: String,
    label: String,
    disabled: bool,
    action: Option<ModPanelAction>,
}

pub struct ModPanelsView {
    document: Document,
    root: HtmlElement,
    rows: Vec<(String, HtmlElement)>,
    current: Rc<RefCell<Vec<ButtonRow>>>,
    actions: Rc<RefCell<Vec<ModPanelAction>>>,
    click: Closure<dyn FnMut(Event)>,
}

impl ModPanelsView {
    pub fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "aside", "mod-panels")?;
        root.set_hidden(true);
        root.set_attribute("aria-label", "Mod content").map_err(|_| "Mod panel label")?;
        let css = element(document, "style", "mod-panels-styles")?;
        css.set_text_content(Some(include_str!("../../../web/mod-panels.css")));
        append(&root, &css)?;
        let current: Rc<RefCell<Vec<ButtonRow>>> = Rc::new(RefCell::new(Vec::new()));
        let actions = Rc::new(RefCell::new(Vec::new()));
        let selected = current.clone();
        let outbox = actions.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let button = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| {
                    target.closest("[data-mod-panel-key]").expect("valid mod button selector")
                });
            let Some(key) = button.and_then(|button| button.get_attribute("data-mod-panel-key"))
            else {
                return;
            };
            if let Some(row) = selected.borrow().iter().find(|row| row.key == key)
                && !row.disabled
                && let Some(action) = &row.action
            {
                outbox.borrow_mut().push(action.clone());
            }
        });
        root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "Could not listen to mod panels")?;
        append(&document.body().ok_or("Mod panels need the game document")?, &root)?;
        Ok(Self { document: document.clone(), root, rows: Vec::new(), current, actions, click })
    }

    /// Source refreshes on accepted runtime and snapshot notifications, including
    /// movement across an authored shop radius or the monument's 100-unit trigger.
    pub fn update(&mut self, snapshot: &ModPanelsSnapshot<'_>) -> Result<(), String> {
        let rows = project_rows(snapshot);
        self.root.set_hidden(rows.is_empty());
        let mut retained = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let button = if let Some(old) = self.rows.iter().position(|(key, _)| *key == row.key) {
                self.rows.remove(old).1
            } else {
                let button = element(&self.document, "button", "")?;
                button.set_attribute("type", "button").map_err(|_| "Mod action button type")?;
                button
                    .set_attribute("data-mod-panel-key", &row.key)
                    .map_err(|_| "Mod action identity")?;
                button
            };
            button.set_text_content(Some(&row.label));
            button
                .toggle_attribute_with_force("disabled", row.disabled)
                .map_err(|_| "Mod stock availability")?;
            // The first child is the retained stylesheet. Avoid moving an already
            // placed focused button on each received host snapshot.
            let next = self.root.children().item(index as u32 + 1);
            if next.as_ref() != Some(button.as_ref()) {
                self.root
                    .insert_before(&button, next.as_ref().map(|next| next.as_ref()))
                    .map_err(|_| "Mod panel button order")?;
            }
            retained.push((row.key.clone(), button));
        }
        for (_, button) in self.rows.drain(..) {
            button.remove();
        }
        self.rows = retained;
        *self.current.borrow_mut() = rows;
        Ok(())
    }

    pub fn hide(&mut self) {
        self.root.set_hidden(true);
        for (_, button) in self.rows.drain(..) {
            button.remove();
        }
        self.current.borrow_mut().clear();
    }

    pub fn reset(&mut self) {
        self.hide();
        self.actions.borrow_mut().clear();
    }

    pub fn take_actions(&mut self) -> Vec<ModPanelAction> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }
}

impl Drop for ModPanelsView {
    fn drop(&mut self) {
        self.root
            .remove_event_listener_with_callback("click", self.click.as_ref().unchecked_ref())
            .expect("remove installed mod panel listener");
        self.root.remove();
    }
}

fn project_rows(snapshot: &ModPanelsSnapshot<'_>) -> Vec<ButtonRow> {
    let Some(runtime) = snapshot.runtime else { return Vec::new() };
    let mut output = Vec::new();
    for spell in rows(runtime.get("spells")) {
        let content_id = text(spell.get("content_id"));
        output.push(ButtonRow {
            key: format!("spell:{content_id}"),
            label: text(spell.get("name")).to_owned(),
            disabled: false,
            action: snapshot.player.as_ref().map(|player| ModPanelAction::Cast {
                content_id: content_id.to_owned(),
                position: player.position,
            }),
        });
    }
    let shops: Vec<_> = rows(runtime.get("shops")).filter(|shop| mounted(shop, snapshot)).collect();
    let shop_stock: Vec<_> = rows(runtime.get("shop_stock")).collect();
    for shop in &shops {
        let content_id = text(shop.get("content_id"));
        for (row, stock) in rows(shop.get("stock")).enumerate() {
            let state = shop_stock.iter().copied().find(|state| {
                state.get("shop_content_id") == shop.get("content_id")
                    && state.get("row").and_then(LuaValue::as_number) == Some(row as f64)
            });
            let remaining =
                number(state.map_or_else(|| stock.get("quantity"), |state| state.get("remaining")));
            let price = number(stock.get("price"));
            output.push(ButtonRow {
                key: format!("shop:{content_id}:{row}"),
                label: format!(
                    "{} · {} gold · {} left",
                    text(shop.get("name")),
                    number_text(price),
                    number_text(remaining)
                ),
                disabled: remaining < 1.0,
                action: Some(ModPanelAction::ShopBuy { content_id: content_id.to_owned(), row }),
            });
        }
    }
    let equipment = snapshot
        .player
        .as_ref()
        .and_then(|player| player.backpack.iter().find(|item| item.kind == HubItemKind::Equipment));
    if let Some(equipment) = equipment {
        for shop in &shops {
            let content_id = text(shop.get("content_id"));
            for (service, row) in rows(shop.get("services")).enumerate() {
                output.push(ButtonRow {
                    key: format!("reforge:{content_id}:{service}"),
                    label: format!(
                        "{} · Reforge · {} gold",
                        text(shop.get("name")),
                        number_text(number(row.get("price")))
                    ),
                    disabled: false,
                    action: Some(ModPanelAction::Reforge {
                        content_id: content_id.to_owned(),
                        item_id: equipment.id,
                        service,
                    }),
                });
            }
        }
    }
    let near_monument = snapshot.player.as_ref().is_some_and(|player| {
        snapshot.scene_objects.iter().any(|object| {
            let x = object.position.x - player.position.x;
            let y = object.position.y - player.position.y;
            object.type_id == 2009 && x * x + y * y < 100.0 * 100.0
        })
    });
    if near_monument {
        for portal in rows(runtime.get("portals")) {
            let id = text(portal.get("id"));
            output.push(ButtonRow {
                key: format!("portal:{id}"),
                label: text(portal.get("prompt")).to_owned(),
                disabled: false,
                action: Some(ModPanelAction::PortalEnter { portal_id: id.to_owned() }),
            });
        }
    }
    output
}

fn mounted(shop: &LuaObject, snapshot: &ModPanelsSnapshot<'_>) -> bool {
    let mount = shop.get("mount");
    let scene = text(mount.and_then(|mount| mount.get("scene")));
    if scene == "hub.courtyard"
        && !matches!(snapshot.world, ModPanelsWorld::Hub { region: Some(HubRegion::Courtyard) })
    {
        return false;
    }
    if scene == "boneyard" && !matches!(snapshot.world, ModPanelsWorld::Boneyard) {
        return false;
    }
    let x = mount.and_then(|mount| mount.get("x")).and_then(LuaValue::as_number);
    let y = mount.and_then(|mount| mount.get("y")).and_then(LuaValue::as_number);
    let (Some(player), Some(x), Some(y)) = (&snapshot.player, x, y) else {
        return scene.is_empty() || scene == "hub.courtyard" || scene == "boneyard";
    };
    let x = x - player.position.x;
    let y = y - player.position.y;
    let radius = number(mount.and_then(|mount| mount.get("radius")));
    let radius = if radius == 0.0 { 120.0 } else { radius };
    x * x + y * y <= radius * radius
}

fn rows(value: Option<&LuaValue>) -> impl Iterator<Item = &LuaObject> {
    value.and_then(LuaValue::as_array).unwrap_or_default().iter().filter_map(LuaValue::as_object)
}

fn text(value: Option<&LuaValue>) -> &str {
    value.and_then(LuaValue::as_str).unwrap_or_default()
}

fn number(value: Option<&LuaValue>) -> f64 {
    value.and_then(LuaValue::as_number).filter(|number| number.is_finite()).unwrap_or(0.0)
}

fn number_text(number: f64) -> String {
    javascript_string(&JsValue::from_f64(number))
}
