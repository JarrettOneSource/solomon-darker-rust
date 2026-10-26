//! Stock Courtyard NPC art from Website `renderer/hub-actors.ts`, `hub-world-scene.ts`,
//! and `tools/student_art.py`. Layers use the same College and Clothes atlases as the scene.

use solomon_assets::SpriteRecord;
use solomon_sim::Vec2;
use solomon_sim::hub::interaction::HubNpcMarkerFrame;
use solomon_sim::hub::npc::{
    HubNpcState, StudentRead, hub_student_head_offset, hub_student_prop_offset,
};
use solomon_sim::hub::painter::{PainterLane, PainterLayer};
use solomon_sim::hub::regions::HubRegion;

use crate::courtyard::{Camera, registered_origin};
use solomon_graphics::{BlendMode, Quad, Renderer, Texture};

const WHITE: [f32; 4] = [1.0; 4];
const STUDENT_PRIMARY: [f32; 4] = [104.0 / 255.0, 130.0 / 255.0, 145.0 / 255.0, 1.0];
const STUDENT_BOOK: [f32; 4] = [126.0 / 255.0, 87.0 / 255.0, 58.0 / 255.0, 1.0];
/// Website student_art.py saturates these five native prop colours by 0.85.
const PROP_COLORS: [[u8; 3]; 5] =
    [[105, 67, 67], [171, 152, 133], [237, 237, 199], [132, 170, 132], [150, 188, 188]];

/// A submitted actor's identity, kept alongside its painter row for this render.
#[derive(Clone, Copy, Debug)]
pub enum HubNpcActor {
    Hagatha,
    Fomentius,
    Annalist,
    Luthacus,
    Skorcha,
    Student(usize),
}

impl HubNpcActor {
    #[must_use]
    pub fn interaction(self) -> Option<solomon_sim::hub::interaction::HubInteractionId> {
        use solomon_sim::hub::interaction::HubInteractionId;
        match self {
            Self::Hagatha => Some(HubInteractionId::Hagatha),
            Self::Fomentius => Some(HubInteractionId::Fomentius),
            Self::Annalist => Some(HubInteractionId::Annalist),
            Self::Luthacus => Some(HubInteractionId::Luthacus),
            Self::Skorcha => Some(HubInteractionId::Skorcha),
            Self::Student(_) => None,
        }
    }
}

pub struct NpcView {
    college_records: Vec<SpriteRecord>,
    clothes_records: Vec<SpriteRecord>,
    college: Texture,
    clothes: Texture,
    shadow: Texture,
}

impl NpcView {
    pub fn new(
        college_records: Vec<SpriteRecord>,
        clothes_records: Vec<SpriteRecord>,
        college: Texture,
        clothes: Texture,
        shadow: Texture,
    ) -> Result<Self, String> {
        if college_records.len() < 525 {
            return Err(format!(
                "College.bundle has {} records; NPCs need 525",
                college_records.len()
            ));
        }
        if clothes_records.len() < 1348 {
            return Err(format!(
                "Clothes.bundle has {} records; students need 1348",
                clothes_records.len()
            ));
        }
        Ok(Self { college_records, clothes_records, college, clothes, shadow })
    }

    pub fn painters(&self, state: &HubNpcState) -> Vec<(PainterLayer, HubNpcActor)> {
        let mut painters = Vec::with_capacity(5 + state.students.students.len());
        for (ordinal, world_y, sort_bias, actor) in [
            (0, 280.0, 0.0, HubNpcActor::Hagatha),
            (1, 664.0, -5.0, HubNpcActor::Fomentius),
            (2, 455.5, 0.0, HubNpcActor::Annalist),
            (3, 449.5, 0.0, HubNpcActor::Luthacus),
        ] {
            painters.push((
                PainterLayer {
                    visible: true,
                    is_insertion: false,
                    manager_lane: PainterLane::Actor,
                    registration_ordinal: ordinal,
                    world_y,
                    sort_bias,
                },
                actor,
            ));
        }
        if let Some(skorcha) = state.skorcha.skorcha.as_ref() {
            painters.push((
                PainterLayer {
                    visible: true,
                    is_insertion: false,
                    manager_lane: PainterLane::Actor,
                    registration_ordinal: 4,
                    world_y: skorcha.position.y,
                    sort_bias: 0.0,
                },
                HubNpcActor::Skorcha,
            ));
        }
        for (index, student) in state.students.students.iter().enumerate() {
            painters.push((
                PainterLayer {
                    visible: true,
                    is_insertion: false,
                    manager_lane: PainterLane::Actor,
                    registration_ordinal: student.registration_ordinal,
                    world_y: student.position.y,
                    sort_bias: 0.0,
                },
                HubNpcActor::Student(index),
            ));
        }
        painters
    }

    pub fn draw_actor(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        state: &HubNpcState,
        actor: HubNpcActor,
    ) {
        match actor {
            HubNpcActor::Hagatha => {
                let root = Vec2::new(1340.0, 280.0);
                self.draw_shadow(renderer, camera, root, 1.25);
                self.college_cell(
                    renderer,
                    camera,
                    517 + state.hagatha.frame(),
                    Cell::at(Vec2::new(root.x - 5.0, root.y), 150.0),
                );
                self.crop(
                    renderer,
                    camera,
                    45,
                    Vec2::new(root.x - 25.0 - 57.0 / 2.0, root.y + 15.0 - 44.0 / 2.0),
                );
                for particle in &state.hagatha.particles {
                    let record = &self.college_records[89 + particle.frame];
                    let [left, top] = registered_origin(record);
                    self.draw(
                        renderer,
                        camera,
                        &self.college,
                        record,
                        QuadPlacement {
                            position: Vec2::new(
                                root.x + particle.offset.x + left * particle.scale,
                                root.y + particle.offset.y + top * particle.scale,
                            ),
                            scale: Vec2::new(particle.scale, particle.scale),
                            tint: [1.0, 1.0, 1.0, particle.alpha() as f32],
                        },
                    );
                }
            }
            HubNpcActor::Fomentius => self.draw_potion_tent(renderer, camera, state),
            HubNpcActor::Annalist => {
                let root = Vec2::new(895.5, 455.5);
                self.draw_shadow(renderer, camera, root, 1.25);
                // The Website crops the two registered 150-cell layers to [9,7,87,96],
                // then anchors that 78x89 composite at its lower centre plus four pixels.
                for index in [0, 47] {
                    let record = &self.college_records[index];
                    let [left, top] = registered_origin(record);
                    self.crop(
                        renderer,
                        camera,
                        index,
                        Vec2::new(root.x - 39.0 + left - 9.0, root.y + 4.0 - 89.0 + top - 7.0),
                    );
                }
            }
            HubNpcActor::Luthacus => {
                let root = Vec2::new(1700.5, 449.5);
                self.draw_shadow(renderer, camera, root, 1.25);
                for index in [10, 126 + state.luthacus.frame()] {
                    self.college_cell(renderer, camera, index, Cell::at(root, 200.0));
                }
            }
            HubNpcActor::Skorcha => {
                let skorcha =
                    state.skorcha.skorcha.as_ref().expect("a painter for present Skorcha");
                self.draw_shadow(renderer, camera, skorcha.position, 1.25);
                let mut cell = Cell::at(skorcha.position, 350.0);
                if skorcha.variant == 1 {
                    cell.scale.x = -1.0;
                }
                self.college_cell(renderer, camera, 510 + skorcha.gesture, cell);
                let hat = skorcha.hat_frame();
                if hat < 4 {
                    self.college_cell(renderer, camera, 513 + hat, cell);
                }
            }
            HubNpcActor::Student(index) => self.draw_student(
                renderer,
                camera,
                &state.students.students.get(index).expect("student painter has a live owner"),
            ),
        }
    }

    pub fn draw_tent_shadow(&self, renderer: &mut Renderer, camera: &Camera) {
        self.draw_world_record(renderer, camera, 33);
    }

    /// Markers occupy the tail of their owner's actor painter. Private scenes use their
    /// corresponding room-bank records through the same simulation frame contract.
    pub fn draw_courtyard_marker(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        frame: HubNpcMarkerFrame,
    ) {
        if !frame.visible || frame.region != HubRegion::Courtyard {
            return;
        }
        let record = &self.college_records[frame.record];
        self.draw(
            renderer,
            camera,
            &self.college,
            record,
            QuadPlacement {
                position: Vec2::new(
                    frame.position.x - f64::from(record.width) / 2.0,
                    frame.position.y - f64::from(record.height) / 2.0,
                ),
                scale: Vec2::new(1.0, 1.0),
                tint: [1.0, 1.0, 1.0, frame.alpha as f32],
            },
        );
    }

    fn draw_student(&self, renderer: &mut Renderer, camera: &Camera, student: &impl StudentRead) {
        self.draw_shadow(renderer, camera, student.position, 1.1);
        let heading = student.heading_index;
        let pose = student.frame_phase.floor() as usize % 5;
        let mut body = Cell::at(student.position, 170.0);
        body.scale = Vec2::new(student.scale, student.scale);
        body.tint = STUDENT_PRIMARY;
        self.clothes_cell(renderer, camera, 868 + pose * 24 + heading, body);
        body.tint = WHITE;
        self.clothes_cell(renderer, camera, 1228 + pose * 24 + heading, body);
        let (white, primary, secondary) = if student.reading {
            ([237, 285], [333, 429], [381, 477])
        } else {
            ([213, 261], [309, 405], [357, 453])
        };
        for base in white {
            self.college_cell(renderer, camera, base + heading, body);
        }
        body.tint = STUDENT_PRIMARY;
        for base in primary {
            self.college_cell(renderer, camera, base + heading, body);
        }
        body.tint = WHITE;
        for base in secondary {
            self.college_cell(renderer, camera, base + heading, body);
        }
        if student.reading {
            body.tint = STUDENT_BOOK;
            self.college_cell(renderer, camera, 189 + heading, body);
        } else {
            for (index, prop) in student.props.iter().enumerate() {
                let offset = hub_student_prop_offset(student.heading, *prop, index);
                body.position =
                    Vec2::new(student.position.x + offset.x, student.position.y + offset.y);
                let [red, green, blue] = PROP_COLORS[prop.palette_index];
                body.tint = [
                    f32::from(red) / 255.0,
                    f32::from(green) / 255.0,
                    f32::from(blue) / 255.0,
                    1.0,
                ];
                self.college_cell(renderer, camera, 165 + heading, body);
            }
        }
        let offset = hub_student_head_offset(student);
        let mut head = Cell::at(
            Vec2::new(student.position.x + offset.x, student.position.y + offset.y),
            170.0,
        );
        head.tint = STUDENT_PRIMARY;
        self.clothes_cell(renderer, camera, 316 + heading, head);
        head.tint = WHITE;
        self.clothes_cell(renderer, camera, 412 + heading, head);
    }

    fn draw_potion_tent(&self, renderer: &mut Renderer, camera: &Camera, state: &HubNpcState) {
        self.draw_world_record(renderer, camera, 34);
        self.crop(renderer, camera, 160 + state.potion.actor.frame(), Vec2::new(1385.0, 618.0));
        self.draw_world_record(renderer, camera, 32);
        let record = &self.college_records[54 + state.potion.balloon_frame()];
        let [left, top] = registered_origin(record);
        let (minimum_x, minimum_y) =
            (54..59).fold((f64::INFINITY, f64::INFINITY), |bounds, index| {
                let origin = registered_origin(&self.college_records[index]);
                (bounds.0.min(origin[0]), bounds.1.min(origin[1]))
            });
        self.crop(
            renderer,
            camera,
            54 + state.potion.balloon_frame(),
            Vec2::new(
                1320.0 + left - minimum_x,
                516.0 + state.potion.balloon_offset_y() + top - minimum_y,
            ),
        );
    }

    fn draw_world_record(&self, renderer: &mut Renderer, camera: &Camera, index: usize) {
        let [left, top] = registered_origin(&self.college_records[index]);
        self.crop(renderer, camera, index, Vec2::new(left + 10.0, top + 60.0));
    }

    fn draw_shadow(&self, renderer: &mut Renderer, camera: &Camera, position: Vec2, scale: f64) {
        let size = 25.0 * scale;
        renderer.draw(
            &self.shadow,
            Quad {
                source: [0.0, 0.0, 25.0, 25.0],
                destination: camera.project(
                    position.x - size / 2.0,
                    position.y - size / 2.0,
                    size,
                    size,
                ),
                tint: [1.0, 1.0, 1.0, 0.62],
                blend: BlendMode::Normal,
            },
        );
    }

    fn college_cell(&self, renderer: &mut Renderer, camera: &Camera, index: usize, cell: Cell) {
        self.draw_cell(renderer, camera, &self.college, &self.college_records[index], cell);
    }

    fn clothes_cell(&self, renderer: &mut Renderer, camera: &Camera, index: usize, cell: Cell) {
        self.draw_cell(renderer, camera, &self.clothes, &self.clothes_records[index], cell);
    }

    fn draw_cell(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        atlas: &Texture,
        record: &SpriteRecord,
        cell: Cell,
    ) {
        let [left, top] = registered_origin(record);
        let cell_left = left + ((cell.size - f64::from(record.logical_width)) / 2.0).floor();
        let cell_top = top + ((cell.size - f64::from(record.logical_height)) / 2.0).floor();
        self.draw(
            renderer,
            camera,
            atlas,
            record,
            QuadPlacement {
                position: Vec2::new(
                    cell.position.x + (cell_left - cell.size / 2.0) * cell.scale.x,
                    cell.position.y + (cell_top - cell.size / 2.0) * cell.scale.y,
                ),
                scale: cell.scale,
                tint: cell.tint,
            },
        );
    }

    fn crop(&self, renderer: &mut Renderer, camera: &Camera, index: usize, position: Vec2) {
        self.draw(
            renderer,
            camera,
            &self.college,
            &self.college_records[index],
            QuadPlacement { position, scale: Vec2::new(1.0, 1.0), tint: WHITE },
        );
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        atlas: &Texture,
        record: &SpriteRecord,
        placement: QuadPlacement,
    ) {
        renderer.draw(
            atlas,
            Quad {
                source: [
                    record.x as f32,
                    record.y as f32,
                    record.width as f32,
                    record.height as f32,
                ],
                destination: camera.project(
                    placement.position.x,
                    placement.position.y,
                    f64::from(record.width) * placement.scale.x,
                    f64::from(record.height) * placement.scale.y,
                ),
                tint: placement.tint,
                blend: BlendMode::Normal,
            },
        );
    }
}

#[derive(Clone, Copy)]
struct Cell {
    position: Vec2,
    size: f64,
    scale: Vec2,
    tint: [f32; 4],
}
impl Cell {
    fn at(position: Vec2, size: f64) -> Self {
        Self { position, size, scale: Vec2::new(1.0, 1.0), tint: WHITE }
    }
}
struct QuadPlacement {
    position: Vec2,
    scale: Vec2,
    tint: [f32; 4],
}
