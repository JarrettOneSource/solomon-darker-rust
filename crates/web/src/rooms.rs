//! The four College private rooms from `renderer/hub-private-room-scene.ts`.
//!
//! The stock-derived images are copied from the pinned Website by `sync-stock-assets.sh`.
//! `tools/hub_room_art.py` and `tools/extract-hub-assets.py` own their composition; sim's
//! room tables keep collider roots, painting anchors and painter roots separate.

use solomon_sim::Vec2;
use solomon_sim::hub::HubWizard;
use solomon_sim::hub::painter::{PainterLane, PainterLayer, hub_painter_order};
use solomon_sim::hub::regions::HubRegion;
use solomon_sim::hub::rooms::{
    HubRoomPropKind, hub_memorator_heading_index, hub_room_flame_transform,
};

use crate::assets;
use crate::courtyard::Camera;
use crate::render::{BlendMode, Quad, Renderer, Texture};

const WHITE: [f32; 4] = [1.0; 4];

pub struct PrivateRoomScenes {
    rooms: [RoomScene; 4],
}

pub struct PrivateRoomFrame<'a> {
    pub local_wizard: &'a HubWizard,
    pub player_painters: &'a [PainterLayer],
    pub tick: u64,
    pub dowser_frame: usize,
}

struct RoomScene {
    region: HubRegion,
    background: Texture,
    props: Texture,
    foreground: Option<Texture>,
    flame: Texture,
    actors: RoomActors,
}

enum RoomActors {
    Mortuary { memorator: Texture, glow: Texture },
    Library { counter: Texture, librarian: Texture, dowser: Texture },
    Storeroom,
    Office { desk: Texture, arch_chancellor: Texture },
}

impl PrivateRoomScenes {
    pub async fn load(renderer: &Renderer) -> Result<Self, String> {
        let mortuary = RoomScene {
            region: HubRegion::Mortuary,
            background: load(renderer, "mortuary-background").await?,
            props: load(renderer, "mortuary-paintings").await?,
            foreground: None,
            flame: load(renderer, "mortuary-flame").await?,
            actors: RoomActors::Mortuary {
                memorator: load(renderer, "memorator").await?,
                glow: load(renderer, "mortuary-memorial-glow").await?,
            },
        };
        let library = RoomScene {
            region: HubRegion::Library,
            background: load(renderer, "library-background").await?,
            props: load(renderer, "library-props").await?,
            foreground: Some(load(renderer, "library-foreground").await?),
            flame: load(renderer, "library-flame").await?,
            actors: RoomActors::Library {
                counter: load(renderer, "librarian").await?,
                librarian: load(renderer, "librarian-frames").await?,
                dowser: load(renderer, "dowser").await?,
            },
        };
        let storeroom = RoomScene {
            region: HubRegion::Storeroom,
            background: load(renderer, "storeroom-background").await?,
            props: load(renderer, "storeroom-props").await?,
            foreground: Some(load(renderer, "storeroom-foreground").await?),
            flame: load(renderer, "storeroom-flame").await?,
            actors: RoomActors::Storeroom,
        };
        let office = RoomScene {
            region: HubRegion::Office,
            background: load(renderer, "office-background").await?,
            props: load(renderer, "office-prop").await?,
            foreground: Some(load(renderer, "office-foreground").await?),
            flame: load(renderer, "office-flame").await?,
            actors: RoomActors::Office {
                desk: load(renderer, "arch-desk").await?,
                arch_chancellor: load(renderer, "arch-chancellor").await?,
            },
        };
        Ok(Self { rooms: [mortuary, library, storeroom, office] })
    }

    pub fn draw(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        frame: &PrivateRoomFrame<'_>,
        draw_wizard: impl FnMut(&mut Renderer, usize),
    ) {
        let room = &self.rooms[usize::from(frame.local_wizard.region.native_id()) - 1];
        room.draw(renderer, camera, frame, draw_wizard);
    }
}

impl RoomScene {
    fn draw(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        frame: &PrivateRoomFrame<'_>,
        mut draw_wizard: impl FnMut(&mut Renderer, usize),
    ) {
        let wizard = frame.local_wizard;
        let tick = frame.tick;
        layer(renderer, camera, &self.background, Vec2::ZERO);
        let props = self.region.room_props();
        let actors = self.region.room_actors();
        let mut painters = Vec::new();
        let mut visible_props = Vec::new();
        for prop in props {
            let Some(visual) = prop.visual else { continue };
            painters.push(PainterLayer {
                visible: true,
                is_insertion: false,
                manager_lane: PainterLane::Actor,
                registration_ordinal: prop.registration_ordinal,
                world_y: visual.painter_y,
                sort_bias: 0.0,
            });
            visible_props.push(visual);
        }
        let actor_base = painters.len();
        painters.extend(actors.iter().map(|actor| PainterLayer {
            visible: true,
            is_insertion: false,
            manager_lane: PainterLane::Actor,
            registration_ordinal: actor.registration_ordinal,
            world_y: actor.painter_y,
            sort_bias: 0.0,
        }));
        let wizard_start = painters.len();
        painters.extend_from_slice(frame.player_painters);
        for index in hub_painter_order(&painters, wizard.position.y) {
            if index >= wizard_start {
                draw_wizard(renderer, index - wizard_start);
                continue;
            }
            if index >= actor_base {
                self.draw_actor(
                    renderer,
                    camera,
                    wizard.position,
                    index - actor_base,
                    frame.dowser_frame,
                );
                continue;
            }
            let visual = visible_props[index];
            match visual.kind {
                HubRoomPropKind::Portrait => draw_frame(
                    renderer,
                    camera,
                    &self.props,
                    visual.frame_index,
                    10,
                    visual.position,
                    true,
                ),
                HubRoomPropKind::RoomFrame => draw_frame(
                    renderer,
                    camera,
                    &self.props,
                    visual.frame_index,
                    3,
                    Vec2::ZERO,
                    false,
                ),
                HubRoomPropKind::RoomLayer => layer(renderer, camera, &self.props, Vec2::ZERO),
            }
        }
        for (index, anchor) in self.region.flame_anchors().iter().enumerate() {
            let transform = hub_room_flame_transform(self.region, tick, index);
            renderer.draw_rotated(
                &self.flame,
                Quad {
                    source: [0.0, 0.0, self.flame.width, self.flame.height],
                    destination: camera.project(
                        anchor.x,
                        anchor.y,
                        f64::from(self.flame.width) * transform.scale_x,
                        f64::from(self.flame.height) * transform.scale_y,
                    ),
                    tint: WHITE,
                    blend: BlendMode::Add,
                },
                transform.rotation,
            );
        }
        if let RoomActors::Mortuary { glow, .. } = &self.actors {
            for _ in 0..3 {
                renderer.draw(
                    glow,
                    Quad {
                        source: [0.0, 0.0, glow.width, glow.height],
                        destination: camera.project(
                            512.0 - f64::from(glow.width) / 2.0,
                            507.0 - f64::from(glow.height) / 2.0,
                            f64::from(glow.width),
                            f64::from(glow.height),
                        ),
                        tint: WHITE,
                        blend: BlendMode::Add,
                    },
                );
            }
        }
        if let Some(foreground) = &self.foreground {
            layer(renderer, camera, foreground, Vec2::ZERO);
        }
        if self.region == HubRegion::Library {
            for x in [16.0, 627.0] {
                renderer.draw_solid(camera.project(x, 801.0, 381.0, 121.0), [0.0, 0.0, 0.0, 1.0]);
            }
        }
    }

    fn draw_actor(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: Vec2,
        index: usize,
        dowser_frame: usize,
    ) {
        let actor = &self.region.room_actors()[index];
        match &self.actors {
            RoomActors::Mortuary { memorator, .. } => draw_frame(
                renderer,
                camera,
                memorator,
                hub_memorator_heading_index(wizard),
                16,
                actor.visual_position,
                true,
            ),
            RoomActors::Library { counter, librarian, dowser } => {
                if index == 0 {
                    layer(renderer, camera, counter, Vec2::new(16.0, 102.5));
                    draw_frame(renderer, camera, librarian, 0, 4, actor.visual_position, true);
                } else {
                    draw_frame(
                        renderer,
                        camera,
                        dowser,
                        dowser_frame,
                        4,
                        actor.visual_position,
                        true,
                    );
                }
            }
            RoomActors::Office { desk, arch_chancellor } => {
                layer(renderer, camera, desk, Vec2::new(102.5, 102.5));
                draw_frame(renderer, camera, arch_chancellor, 0, 3, actor.visual_position, true);
            }
            RoomActors::Storeroom => unreachable!("the storeroom has no fixed staff"),
        }
    }
}

async fn load(renderer: &Renderer, name: &str) -> Result<Texture, String> {
    renderer.upload(&assets::load_image(&format!("assets/stock/hub-room-{name}.png")).await?)
}

fn layer(renderer: &mut Renderer, camera: &Camera, texture: &Texture, position: Vec2) {
    draw_frame(renderer, camera, texture, 0, 1, position, false);
}

fn draw_frame(
    renderer: &mut Renderer,
    camera: &Camera,
    texture: &Texture,
    index: usize,
    count: usize,
    position: Vec2,
    centered: bool,
) {
    let width = texture.width / count as f32;
    let height = texture.height;
    let offset = if centered {
        Vec2::new(f64::from(width) / 2.0, f64::from(height) / 2.0)
    } else {
        Vec2::ZERO
    };
    renderer.draw(
        texture,
        Quad {
            source: [index as f32 * width, 0.0, width, height],
            destination: camera.project(
                position.x - offset.x,
                position.y - offset.y,
                f64::from(width),
                f64::from(height),
            ),
            tint: WHITE,
            blend: BlendMode::Normal,
        },
    );
}
