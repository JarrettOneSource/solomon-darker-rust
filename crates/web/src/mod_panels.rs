//! GPU `mod-ui/ModPanels.tsx`; mounted actions use accepted host IDs and positions.

use crate::gpu_ui::UiFrame;
use crate::ui::javascript_string;
use egui::{Color32, FontFamily, FontId, Id, Stroke, vec2};
use solomon_sim::Vec2;
use solomon_sim::economy::{HubInventoryItem, HubItemKind};
use solomon_sim::hub::regions::HubRegion;
use solomon_sim::modding::{LuaObject, LuaValue};
use wasm_bindgen::JsValue;
use web_sys::Document;

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
    rows: Vec<ButtonRow>,
    actions: Vec<ModPanelAction>,
}
impl ModPanelsView {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Middle, egui::Id::unique("mod/panels"))
    }

    pub fn new(_document: &Document) -> Result<Self, String> {
        Ok(Self { rows: Vec::new(), actions: Vec::new() })
    }
    pub fn update(&mut self, snapshot: &ModPanelsSnapshot<'_>) -> Result<(), String> {
        self.rows = project_rows(snapshot);
        Ok(())
    }
    pub fn hide(&mut self) {
        self.rows.clear();
    }
    pub fn reset(&mut self) {
        self.hide();
        self.actions.clear();
    }
    pub fn take_actions(&mut self) -> Vec<ModPanelAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        if self.rows.is_empty() {
            return Ok(());
        }
        let ctx = frame.ctx.clone();
        egui::Area::new(self.layer_id().id)
            .order(self.layer_id().order)
            .anchor(egui::Align2::RIGHT_BOTTOM, vec2(-16.0, -80.0))
            .show(&ctx, |ui| -> Result<(), String> {
                ui.set_width(256.0_f32.min(ctx.content_rect().width() * 0.42));
                egui::ScrollArea::vertical()
                    .id_salt("mod/panels-scroll")
                    .max_height(ctx.content_rect().height() * 0.4)
                    .show(ui, |ui| -> Result<(), String> {
                        ui.spacing_mut().item_spacing.y = 5.6;
                        for row in &self.rows {
                            let width = ui.available_width();
                            let label = crate::gpu_ui::font_run_with_options(
                                frame.ctx,
                                FontId::new(12.0, FontFamily::Name("Cinzel Semibold".into())),
                                Color32::from_rgb(230, 217, 178),
                                &row.label,
                                crate::gpu_ui::FontRunOptions {
                                    wrap_width: Some((width - 22.8).max(1.0)),
                                    line_height: Some(14.4),
                                    ..Default::default()
                                },
                            )?;
                            // Source 7.2px/10.4px padding inside its 1px border.
                            let (bounds, response) = ui
                                .add_enabled_ui(!row.disabled, |ui| {
                                    ui.allocate_exact_size(
                                        vec2(width, label.size().y + 16.4),
                                        egui::Sense::click(),
                                    )
                                })
                                .inner;
                            ui.painter().rect_filled(
                                bounds,
                                0.0,
                                Color32::from_rgba_unmultiplied(10, 12, 17, 224),
                            );
                            ui.painter().rect_stroke(
                                bounds,
                                0.0,
                                Stroke::new(
                                    1.0,
                                    Color32::from_rgba_unmultiplied(210, 190, 139, 191),
                                ),
                                egui::StrokeKind::Inside,
                            );
                            label.paint(frame, ui.painter(), bounds.min + vec2(11.4, 8.2), 1.0)?;
                            frame.record(
                                &format!("mod/panel:{}", row.key),
                                &row.label,
                                "button",
                                &response,
                            );
                            if response.clicked()
                                && let Some(action) = &row.action
                            {
                                self.actions.push(action.clone());
                            }
                        }
                        Ok(())
                    })
                    .inner?;
                Ok(())
            })
            .inner?;
        Ok(())
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
