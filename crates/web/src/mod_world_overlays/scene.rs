//! GPU authored room map and host controls from `mod-ui/ModSceneOverlay.tsx`.

use crate::gpu_ui::UiFrame;
use egui::{Color32, FontFamily, FontId, Id, Order, Rect, RichText, Stroke, pos2, vec2};
use solomon_protocol::hosted::ModAction;
use solomon_sim::modding::presentation::{mod_scene_color, mod_scene_number, project_mod_scene};
use solomon_sim::modding::{LuaObject, LuaValue};
use web_sys::Document;

pub struct ModViewAction {
    pub action: ModAction,
    pub target: String,
    pub arguments: LuaObject,
}
pub struct ModScenePartyMember<'a> {
    pub id: &'a str,
    pub display_name: &'a str,
}
pub struct ModSceneOverlayFrame<'a> {
    pub runtime: Option<&'a LuaObject>,
    pub owner_id: &'a str,
    pub is_host: bool,
    pub party: &'a [ModScenePartyMember<'a>],
}
struct Wall {
    rect: Rect,
    color: Color32,
}
struct Prop {
    center: egui::Pos2,
    radius: f32,
    color: Color32,
    label: String,
}
struct Room {
    content_id: String,
    index: usize,
    count: usize,
    is_host: bool,
    name: String,
    description: String,
    size: egui::Vec2,
    floor: Color32,
    walls: Vec<Wall>,
    props: Vec<Prop>,
    party: Vec<String>,
}
pub struct ModSceneOverlay {
    room: Option<Room>,
    actions: Vec<ModViewAction>,
}
impl ModSceneOverlay {
    pub(super) fn new(_document: &Document) -> Result<Self, String> {
        Ok(Self { room: None, actions: Vec::new() })
    }
    pub fn reset(&mut self) {
        self.room = None;
        self.actions.clear();
    }
    pub fn take_actions(&mut self) -> Vec<ModViewAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn refresh(&mut self, frame: &ModSceneOverlayFrame<'_>) -> Result<(), String> {
        let Some(scene) = project_mod_scene(frame.runtime, frame.owner_id) else {
            self.reset();
            return Ok(());
        };
        let room = &scene.rooms[scene.room_index];
        self.room = Some(Room {
            content_id: scene.content_id.to_owned(),
            index: scene.room_index,
            count: scene.rooms.len(),
            is_host: frame.is_host,
            name: room.name.to_owned(),
            description: room.description.to_owned(),
            size: vec2(room.width as f32, room.height as f32),
            floor: color(room.floor)?,
            walls: room
                .walls
                .iter()
                .map(|wall| {
                    Ok(Wall {
                        rect: Rect::from_min_size(
                            pos2(
                                mod_scene_number(wall.get("x"), 0.0) as f32,
                                mod_scene_number(wall.get("y"), 0.0) as f32,
                            ),
                            vec2(
                                mod_scene_number(wall.get("width"), 80.0).max(4.0) as f32,
                                mod_scene_number(wall.get("height"), 24.0).max(4.0) as f32,
                            ),
                        ),
                        color: color(mod_scene_color(wall.get("color"), "#51465f"))?,
                    })
                })
                .collect::<Result<_, String>>()?,
            props: room
                .props
                .iter()
                .map(|prop| {
                    Ok(Prop {
                        center: pos2(
                            mod_scene_number(prop.get("x"), room.width / 2.0) as f32,
                            mod_scene_number(prop.get("y"), room.height / 2.0) as f32,
                        ),
                        radius: mod_scene_number(prop.get("radius"), 18.0).max(8.0) as f32,
                        color: color(mod_scene_color(prop.get("color"), "#a68bc6"))?,
                        label: prop
                            .get("label")
                            .and_then(LuaValue::as_str)
                            .or_else(|| prop.get("kind").and_then(LuaValue::as_str))
                            .unwrap_or("prop")
                            .to_owned(),
                    })
                })
                .collect::<Result<_, String>>()?,
            party: frame.party.iter().map(|member| member.display_name.to_owned()).collect(),
        });
        Ok(())
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some(room) = &self.room else { return Ok(()) };
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        let small = screen.width() <= 700.0;
        let padding = if small { 9.6 } else { 16.0 };
        let width = (screen.width() - padding * 2.0).min(1120.0);
        let left = (screen.width() - width) / 2.0;
        let header = Rect::from_min_size(pos2(left, padding), vec2(width, 56.0));
        let footer = Rect::from_min_size(
            pos2(left, screen.height() - padding - if small { 90.0 } else { 44.0 }),
            vec2(width, if small { 90.0 } else { 44.0 }),
        );
        let viewport = Rect::from_min_max(
            pos2(left, header.bottom() + 12.0),
            pos2(left + width, footer.top() - 12.0),
        );
        let mut direction = None;
        egui::Area::new(Id::unique("mod/scene"))
            .order(Order::Foreground)
            .fixed_pos(egui::Pos2::ZERO)
            .show(&ctx, |ui| {
                ui.set_min_size(screen.size());
                ui.interact(screen, Id::unique("mod/scene-block"), egui::Sense::click());
                ui.painter().rect_filled(screen, 0.0, Color32::from_rgb(3, 4, 7));
                ui.scope_builder(egui::UiBuilder::new().max_rect(header), |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(copy(&room.name, 17.6));
                            if !room.description.is_empty() {
                                ui.label(
                                    RichText::new(&room.description)
                                        .font(FontId::new(
                                            12.48,
                                            FontFamily::Name("Alegreya".into()),
                                        ))
                                        .color(Color32::from_rgb(191, 179, 200)),
                                );
                            }
                        });
                        let response = ui.label(copy(
                            &format!("Room {} / {}", room.index + 1, room.count),
                            12.0,
                        ));
                        frame.record(
                            "mod/scene-room-count",
                            &format!("Room {} / {}", room.index + 1, room.count),
                            "status",
                            &response,
                        );
                    });
                });
                ui.painter().rect_filled(viewport, 0.0, Color32::from_rgb(8, 7, 11));
                ui.painter().rect_stroke(
                    viewport,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(170, 149, 97)),
                    egui::StrokeKind::Inside,
                );
                let scale = (viewport.width() / room.size.x).min(viewport.height() / room.size.y);
                let map = Rect::from_center_size(viewport.center(), room.size * scale);
                let painter = ui.painter().with_clip_rect(viewport);
                let point = |point: egui::Pos2| map.min + point.to_vec2() * scale;
                painter.rect_filled(map, 0.0, room.floor);
                painter.rect_stroke(
                    map.shrink(4.0 * scale),
                    0.0,
                    Stroke::new(8.0 * scale, Color32::from_rgb(139, 123, 157)),
                    egui::StrokeKind::Middle,
                );
                for wall in &room.walls {
                    painter.rect_filled(
                        Rect::from_min_size(point(wall.rect.min), wall.rect.size() * scale),
                        0.0,
                        wall.color,
                    );
                }
                for prop in &room.props {
                    let center = point(prop.center);
                    painter.circle_filled(center, prop.radius * scale, prop.color);
                    label(&painter, center + vec2(0.0, 34.0 * scale), &prop.label, scale);
                }
                for (index, name) in room.party.iter().enumerate() {
                    let center = point(pos2(
                        room.size.x / 2.0
                            + (index as f32 - (room.party.len() as f32 - 1.0) / 2.0) * 44.0,
                        room.size.y * 0.72,
                    ));
                    painter.circle(
                        center,
                        16.0 * scale,
                        Color32::from_rgb(177, 139, 211),
                        Stroke::new(3.0 * scale, Color32::from_rgb(242, 231, 200)),
                    );
                    label(&painter, center + vec2(0.0, 32.0 * scale), name, scale);
                }
                frame.record_rect(
                    "mod/scene-map",
                    &format!("{} map", room.name),
                    "img",
                    viewport,
                    true,
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(footer), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (id, text, next, enabled) in [
                            ("previous", "Previous room", -1, room.is_host && room.index > 0),
                            ("next", "Next room", 1, room.is_host && room.index + 1 < room.count),
                            ("return", "Return to the Boneyard", 0, room.is_host),
                        ] {
                            let response = ui.add_enabled(
                                enabled,
                                egui::Button::new(copy(text, 12.0))
                                    .fill(Color32::from_rgb(25, 23, 34))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(210, 190, 139)))
                                    .corner_radius(0.0)
                                    .min_size(vec2(
                                        if small { (width - 12.0) / 2.0 } else { 180.0 },
                                        40.0,
                                    )),
                            );
                            frame.record(&format!("mod/scene:{id}"), text, "button", &response);
                            if response.clicked() {
                                direction = Some(next);
                            }
                        }
                    });
                    if !room.is_host {
                        ui.label(copy("Waiting for the party leader…", 11.52));
                    }
                });
                frame.record_rect("mod/scene", &room.name, "dialog", screen, true);
            });
        if let Some(direction) = direction {
            if !room.is_host {
                return Ok(());
            }
            let mut arguments = LuaObject::new();
            let action = if direction == 0 {
                ModAction::SceneReturn
            } else {
                let index = room.index as i64 + i64::from(direction);
                if index < 0 || index as usize >= room.count {
                    return Ok(());
                }
                arguments.insert("room".into(), LuaValue::Number(index as f64));
                ModAction::SceneRoom
            };
            self.actions.push(ModViewAction { action, target: room.content_id.clone(), arguments });
        }
        Ok(())
    }
}
fn color(value: &str) -> Result<Color32, String> {
    Color32::from_hex(value).map_err(|error| format!("Invalid admitted room color: {error:?}"))
}
fn copy(text: &str, size: f32) -> RichText {
    RichText::new(text)
        .font(FontId::new(size, FontFamily::Name("Cinzel".into())))
        .color(Color32::from_rgb(230, 217, 178))
}
fn label(painter: &egui::Painter, position: egui::Pos2, text: &str, scale: f32) {
    let font = FontId::new(14.0 * scale, FontFamily::Name("Alegreya".into()));
    for offset in [vec2(-1.5, 0.0), vec2(1.5, 0.0), vec2(0.0, -1.5), vec2(0.0, 1.5)] {
        painter.text(
            position + offset * scale,
            egui::Align2::CENTER_BOTTOM,
            text,
            font.clone(),
            Color32::from_rgb(8, 7, 11),
        );
    }
    painter.text(
        position,
        egui::Align2::CENTER_BOTTOM,
        text,
        font,
        Color32::from_rgb(231, 221, 196),
    );
}
