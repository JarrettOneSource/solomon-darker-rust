//! GPU authored room map and host controls from `mod-ui/ModSceneOverlay.tsx`.

use crate::gpu_ui::{
    FontRunOptions, GroupEffect, ImageShadow, NativeFontStroke, UiFrame, font_run,
    font_run_with_options,
};
use egui::{Color32, FontFamily, FontId, Id, Order, Rect, Stroke, pos2, vec2};
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
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(Order::Foreground, Id::unique("mod/scene"))
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some(room) = &self.room else { return Ok(()) };
        frame
            .input_scope(&format!("mod/scene:{}:{}", room.content_id, room.index), self.layer_id());
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        let small = screen.width() <= 700.0;
        let padding = if small { 9.6 } else { 16.0 };
        let width = (screen.width() - padding * 2.0).min(1120.0);
        let left = screen.center().x - width / 2.0;
        let count_text = format!("Room {} / {}", room.index + 1, room.count);
        let count = font_run_with_options(
            frame.ctx,
            FontId::new(16.0, FontFamily::Name("Alegreya".into())),
            Color32::from_rgb(230, 217, 178),
            &count_text,
            FontRunOptions { line_height: Some(24.0), ..Default::default() },
        )?;
        let heading_width = (width - count.size().x - 10.4).max(1.0);
        let title = font_run(
            frame.ctx,
            FontId::new(17.6, FontFamily::Name("Cinzel Bold".into())),
            Color32::from_rgb(230, 217, 178),
            &room.name,
            Some(heading_width),
        )?;
        let description = if room.description.is_empty() {
            None
        } else {
            Some(font_run_with_options(
                frame.ctx,
                FontId::new(12.48, FontFamily::Name("Alegreya".into())),
                Color32::from_rgb(191, 179, 200),
                &room.description,
                FontRunOptions {
                    wrap_width: Some(heading_width),
                    line_height: Some(12.48 * 1.35),
                    ..Default::default()
                },
            )?)
        };
        let heading_height = title.size().y + description.as_ref().map_or(0.0, |run| run.size().y);
        let header_height = heading_height.max(count.size().y);
        let buttons = [
            ("previous", "Previous room", -1, room.is_host && room.index > 0),
            ("next", "Next room", 1, room.is_host && room.index + 1 < room.count),
            ("return", "Return to the Boneyard", 0, room.is_host),
        ];
        let runs = buttons
            .iter()
            .enumerate()
            .map(|(index, (_, label, _, _))| {
                font_run(
                    frame.ctx,
                    FontId::new(12.0, FontFamily::Name("Cinzel Semibold".into())),
                    Color32::from_rgb(242, 231, 200),
                    label,
                    if small {
                        Some(if index == 2 { width - 27.6 } else { (width - 10.4) / 2.0 - 27.6 })
                    } else {
                        None
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let button_sizes: Vec<_> = runs.iter().map(|run| run.size() + vec2(27.6, 21.2)).collect();
        let first_height = button_sizes[0].y.max(button_sizes[1].y);
        let footer_height = if small {
            first_height + 10.4 + button_sizes[2].y
        } else {
            button_sizes.iter().map(|size| size.y).fold(0.0, f32::max)
        };
        let header =
            Rect::from_min_size(pos2(left, screen.top() + padding), vec2(width, header_height));
        let footer = Rect::from_min_size(
            pos2(left, screen.bottom() - padding - footer_height),
            vec2(width, footer_height),
        );
        let viewport = Rect::from_min_max(
            pos2(left, header.bottom() + 12.0),
            pos2(left + width, (footer.top() - 12.0).max(header.bottom() + 14.0)),
        );
        let mut direction = None;
        egui::Area::new(self.layer_id().id)
            .fade_in(false)
            .order(self.layer_id().order)
            .fixed_pos(screen.min)
            .show(&ctx, |ui| -> Result<(), String> {
                ui.memory_mut(|memory| memory.set_modal_layer(ui.layer_id()));
                ui.set_min_size(screen.size());
                ui.interact(screen, Id::unique("mod/scene-block"), egui::Sense::CLICK);
                paint_background(ui.painter(), screen);
                let heading_top =
                    header.top() + if small { 0.0 } else { (header_height - heading_height) / 2.0 };
                title.paint(frame, ui.painter(), pos2(left, heading_top), 1.0)?;
                frame.record_rect(
                    "mod/scene-title",
                    &room.name,
                    "heading",
                    Rect::from_min_size(pos2(left, heading_top), title.size()),
                    true,
                );
                if let Some(description) = &description {
                    let origin = pos2(left, heading_top + title.size().y);
                    description.paint(frame, ui.painter(), origin, 1.0)?;
                    frame.record_rect(
                        "mod/scene-description",
                        &room.description,
                        "text",
                        Rect::from_min_size(origin, description.size()),
                        true,
                    );
                }
                let count_origin = pos2(
                    header.right() - count.size().x,
                    header.top() + if small { 0.0 } else { (header_height - count.size().y) / 2.0 },
                );
                count.paint(frame, ui.painter(), count_origin, 1.0)?;
                frame.record_rect(
                    "mod/scene-room-count",
                    &count_text,
                    "status",
                    Rect::from_min_size(count_origin, count.size()),
                    true,
                );
                frame.paint_outer_shadow(
                    ui.painter(),
                    ImageShadow {
                        offset: egui::Vec2::ZERO,
                        blur: 15.0,
                        color: Color32::from_black_alpha(179),
                    },
                    |caster| {
                        caster.rect_filled(viewport, 0.0, Color32::WHITE);
                        Ok(())
                    },
                )?;
                ui.painter().rect_filled(viewport, 0.0, Color32::from_rgb(8, 7, 11));
                let inner = viewport.shrink(1.0);
                let scale = (inner.width() / room.size.x).min(inner.height() / room.size.y);
                if scale > 0.0 {
                    let map = Rect::from_center_size(inner.center(), room.size * scale);
                    let painter = ui.painter().with_clip_rect(inner);
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
                        label(
                            frame,
                            &painter,
                            center + vec2(0.0, 34.0 * scale),
                            &prop.label,
                            scale,
                        )?;
                    }
                    for (index, name) in room.party.iter().enumerate() {
                        let center = point(pos2(
                            room.size.x / 2.0
                                + (index as f32 - (room.party.len() as f32 - 1.0) / 2.0) * 44.0,
                            room.size.y * 0.72,
                        ));
                        painter.circle_filled(
                            center,
                            16.0 * scale,
                            Color32::from_rgb(177, 139, 211),
                        );
                        painter.circle_stroke(
                            center,
                            16.0 * scale,
                            Stroke::new(3.0 * scale, Color32::from_rgb(242, 231, 200)),
                        );
                        label(frame, &painter, center + vec2(0.0, 32.0 * scale), name, scale)?;
                    }
                }
                ui.painter().rect_stroke(
                    viewport,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(170, 149, 97)),
                    egui::StrokeKind::Inside,
                );
                frame.record_rect(
                    "mod/scene-map",
                    &format!("{} map", room.name),
                    "img",
                    viewport,
                    true,
                );
                let spare = width - button_sizes.iter().map(|size| size.x).sum::<f32>();
                let mut button_left = footer.left();
                for (index, ((id, text, next, enabled), run)) in
                    buttons.iter().zip(&runs).enumerate()
                {
                    let bounds = if small {
                        if index == 2 {
                            Rect::from_min_size(
                                pos2(footer.left(), footer.top() + first_height + 10.4),
                                vec2(width, button_sizes[2].y),
                            )
                        } else {
                            Rect::from_min_size(
                                pos2(
                                    footer.left() + index as f32 * (width + 10.4) / 2.0,
                                    footer.top(),
                                ),
                                vec2((width - 10.4) / 2.0, first_height),
                            )
                        }
                    } else {
                        let bounds = Rect::from_min_size(
                            pos2(
                                button_left,
                                footer.top() + (footer_height - button_sizes[index].y) / 2.0,
                            ),
                            button_sizes[index],
                        );
                        button_left = bounds.right() + spare / 2.0;
                        bounds
                    };
                    let response = ui
                        .add_enabled_ui(*enabled, |ui| {
                            ui.interact(
                                bounds,
                                Id::unique(("mod/scene-button", id)),
                                egui::Sense::click(),
                            )
                        })
                        .inner;
                    frame.paint_group(
                        ui.painter(),
                        GroupEffect {
                            opacity: if *enabled { 1.0 } else { 0.38 },
                            ..Default::default()
                        },
                        |frame, painter| {
                            painter.rect_filled(bounds, 0.0, Color32::from_rgb(25, 23, 34));
                            painter.rect_stroke(
                                bounds,
                                0.0,
                                Stroke::new(1.0, Color32::from_rgb(210, 190, 139)),
                                egui::StrokeKind::Inside,
                            );
                            run.paint(frame, painter, bounds.center() - run.size() / 2.0, 1.0)
                        },
                    )?;
                    frame.record(&format!("mod/scene:{id}"), text, "button", &response);
                    if response.clicked() {
                        direction = Some(*next);
                    }
                }
                if !room.is_host {
                    let waiting = font_run(
                        frame.ctx,
                        FontId::new(11.52, FontFamily::Name("Alegreya".into())),
                        Color32::from_rgb(230, 217, 178),
                        "Waiting for the party leader…",
                        None,
                    )?;
                    let origin = screen.right_bottom() - vec2(16.0, 16.0) - waiting.size();
                    waiting.paint(frame, ui.painter(), origin, 1.0)?;
                    frame.record_rect(
                        "mod/scene-waiting",
                        "Waiting for the party leader…",
                        "status",
                        Rect::from_min_size(origin, waiting.size()),
                        true,
                    );
                }
                frame.record_rect("mod/scene", &room.name, "dialog", screen, true);
                Ok(())
            })
            .inner?;
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
fn label(
    frame: &UiFrame<'_>,
    painter: &egui::Painter,
    baseline: egui::Pos2,
    text: &str,
    scale: f32,
) -> Result<(), String> {
    let run = font_run_with_options(
        frame.ctx,
        FontId::new(14.0 * scale, FontFamily::Name("Alegreya".into())),
        Color32::from_rgb(231, 221, 196),
        text,
        FontRunOptions {
            stroke: Some(NativeFontStroke {
                width: 3.0 * scale,
                color: Color32::from_rgb(8, 7, 11),
            }),
            ..Default::default()
        },
    )?;
    run.paint(frame, painter, baseline - vec2(run.size().x / 2.0, run.baseline()), 1.0)
}

fn paint_background(painter: &egui::Painter, screen: Rect) {
    let edge = Color32::from_rgb(3, 4, 7);
    painter.rect_filled(screen, 0.0, edge);
    let mut gradient = egui::Mesh::default();
    gradient.colored_vertex(screen.center(), Color32::from_rgb(37, 31, 48));
    let radius = screen.size().length() * 0.35;
    for index in 0..=128 {
        let angle = std::f32::consts::TAU * index as f32 / 128.0;
        gradient.colored_vertex(screen.center() + vec2(angle.cos(), angle.sin()) * radius, edge);
        if index > 0 {
            gradient.add_triangle(0, index, index + 1);
        }
    }
    painter.with_clip_rect(screen).add(egui::Shape::mesh(gradient));
}
