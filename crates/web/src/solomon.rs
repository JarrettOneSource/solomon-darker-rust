//! Thin GPU view of the retained opening encounter (`renderer/boneyard-solomon-view.ts`).

use solomon_assets::SpriteRecord;
use solomon_sim::Vec2;
use solomon_sim::boneyard::SolomonDig;
use solomon_sim::boneyard::encounter::SolomonEncounter;
use solomon_sim::boneyard::solomon_presentation::{
    SOLOMON_DIRT_DRAW_PASSES, SolomonBodyBank, SolomonClipRect, SolomonDirtPaintState,
    SolomonPainter, SolomonPainterKind, SolomonSetPieceLighting, SolomonVisualState,
    boneyard_solomon_painters, boneyard_solomon_visual_state,
};
use solomon_sim::hub::painter::PainterLayer;
use solomon_sim::painter::PainterRegistration;

use crate::assets::load_image;
use crate::courtyard::Camera;
use crate::render::{BlendMode, Quad, Renderer, Texture};

pub struct SolomonViewFrame {
    pub tick: u64,
    pub lantern_position: Option<Vec2>,
    pub lantern_registration: PainterRegistration,
    pub solomon_registration: PainterRegistration,
    pub lighting: SolomonSetPieceLighting,
}

struct StockSprite {
    texture: Texture,
    record: SpriteRecord,
}

struct PreparedSolomon {
    position: Vec2,
    lantern_position: Option<Vec2>,
    visual: SolomonVisualState,
    lighting: SolomonSetPieceLighting,
}

pub struct SolomonView {
    dig: Texture,
    encounter: Texture,
    flydirt: Texture,
    grave_mark: StockSprite,
    lantern: StockSprite,
    dirt: SolomonDirtPaintState,
    prepared: Option<PreparedSolomon>,
    painters: Vec<SolomonPainter>,
}

impl SolomonView {
    /// Share existing atlas uploads; only the native composited Solomon sheets are new.
    pub async fn load(
        renderer: &Renderer,
        bad_guys_records: &[SpriteRecord],
        bad_guys_texture: Texture,
        dead_hawg_records: &[SpriteRecord],
        dead_hawg_texture: Texture,
    ) -> Result<Self, String> {
        let dig = load_image("assets/stock/anim-solomon-dig.png").await?;
        let encounter = load_image("assets/stock/anim-solomon-encounter.png").await?;
        let flydirt = load_image("assets/stock/solomon-flydirt.png").await?;
        Ok(Self {
            dig: renderer.upload(&dig)?,
            encounter: renderer.upload(&encounter)?,
            flydirt: renderer.upload(&flydirt)?,
            grave_mark: StockSprite {
                texture: dead_hawg_texture,
                record: dead_hawg_records
                    .get(13)
                    .ok_or("missing Solomon grave mark record")?
                    .clone(),
            },
            lantern: StockSprite {
                texture: bad_guys_texture,
                record: bad_guys_records.get(34).ok_or("missing Solomon lantern record")?.clone(),
            },
            dirt: SolomonDirtPaintState::default(),
            prepared: None,
            painters: Vec::new(),
        })
    }

    /// Keep uploaded art across runs, discarding only paint-time roots and event cursors.
    pub fn reset(&mut self) {
        self.dirt.reset();
        self.prepared = None;
        self.painters.clear();
    }

    pub fn prepare(
        &mut self,
        dig: &SolomonDig,
        encounter: Option<&SolomonEncounter>,
        frame: &SolomonViewFrame,
    ) {
        self.dirt.prepare(encounter, frame.tick);
        self.painters = boneyard_solomon_painters(
            dig,
            encounter,
            frame.lantern_registration,
            frame.solomon_registration,
            frame.lantern_position,
        );
        self.prepared = Some(PreparedSolomon {
            position: encounter.map_or(dig.position, |encounter| encounter.position),
            lantern_position: frame.lantern_position,
            visual: boneyard_solomon_visual_state(encounter, dig, frame.tick),
            lighting: frame.lighting,
        });
    }

    pub fn painters(&self) -> impl Iterator<Item = (SolomonPainterKind, PainterLayer)> + '_ {
        self.painters
            .iter()
            .map(|painter| (painter.kind, painter.registration.layer(painter.world_y, 0.0)))
    }

    pub fn draw(&self, renderer: &mut Renderer, camera: &Camera, kind: SolomonPainterKind) {
        let Some(prepared) = &self.prepared else {
            return;
        };
        if kind == SolomonPainterKind::Lantern {
            if let Some(position) = prepared.lantern_position {
                draw_registered(
                    renderer,
                    camera,
                    &self.lantern,
                    position,
                    prepared.lighting.lantern_tint,
                );
            }
            return;
        }
        let visual = prepared.visual;
        if !visual.visible {
            return;
        }
        let (texture, column, row) = match visual.body_bank {
            SolomonBodyBank::Dig => (&self.dig, visual.body_pose, 0),
            SolomonBodyBank::Dialogue => (&self.encounter, visual.direction, 0),
            SolomonBodyBank::Walk => (&self.encounter, visual.direction, 4 + visual.body_pose),
        };
        let body_position =
            Vec2::new(prepared.position.x - 100.0, prepared.position.y - 100.0 + visual.offset_y);
        draw_clipped_cell(
            renderer,
            camera,
            texture,
            column,
            row,
            body_position,
            visual.clip_rect_world,
            prepared.lighting.body_tint,
        );
        if let Some(pose) = visual.mouth_pose {
            draw_clipped_cell(
                renderer,
                camera,
                &self.encounter,
                visual.direction,
                1 + pose,
                body_position,
                visual.clip_rect_world,
                prepared.lighting.body_tint,
            );
        }
        // The mask belongs to body and mouth only. The planted grave and thrown dirt are
        // later actor children and remain outside it, preserving the source's child order.
        if visual.grave_mark_visible {
            draw_registered(
                renderer,
                camera,
                &self.grave_mark,
                Vec2::new(prepared.position.x - 10.0, prepared.position.y - 113.0),
                prepared.lighting.body_tint,
            );
        }
        for particle in self.dirt.particles() {
            let state = particle.state;
            let quad = Quad {
                source: [0.0, 0.0, self.flydirt.width, self.flydirt.height],
                destination: camera.project(
                    state.position.x - f64::from(self.flydirt.width) * 0.5,
                    state.position.y - f64::from(self.flydirt.height) * 0.5,
                    f64::from(self.flydirt.width),
                    f64::from(self.flydirt.height),
                ),
                tint: tint(prepared.lighting.dirt_tint, state.alpha),
                blend: BlendMode::Normal,
            };
            for _ in 0..SOLOMON_DIRT_DRAW_PASSES {
                renderer.draw_rotated(
                    &self.flydirt,
                    quad,
                    state.heading_degrees * std::f64::consts::PI / 180.0,
                );
            }
        }
    }
}

fn draw_registered(
    renderer: &mut Renderer,
    camera: &Camera,
    sprite: &StockSprite,
    position: Vec2,
    color: u32,
) {
    let record = &sprite.record;
    let width = f64::from(record.width);
    let height = f64::from(record.height);
    let anchor_x = width * 0.5 - f64::from(record.center_x);
    let anchor_y = height * 0.5 - f64::from(record.center_y);
    renderer.draw(
        &sprite.texture,
        Quad {
            source: [record.x as f32, record.y as f32, record.width as f32, record.height as f32],
            destination: camera.project(
                position.x - anchor_x,
                position.y - anchor_y,
                width,
                height,
            ),
            tint: tint(color, 1.0),
            blend: BlendMode::Normal,
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_clipped_cell(
    renderer: &mut Renderer,
    camera: &Camera,
    texture: &Texture,
    column: usize,
    row: usize,
    position: Vec2,
    clip: Option<SolomonClipRect>,
    color: u32,
) {
    let left = clip.map_or(position.x, |clip| position.x.max(clip.x));
    let top = clip.map_or(position.y, |clip| position.y.max(clip.y));
    let right =
        clip.map_or(position.x + 200.0, |clip| (position.x + 200.0).min(clip.x + clip.width));
    let bottom =
        clip.map_or(position.y + 200.0, |clip| (position.y + 200.0).min(clip.y + clip.height));
    if right <= left || bottom <= top {
        return;
    }
    let width = right - left;
    let height = bottom - top;
    // The sprites are unrotated 200-unit cells, so intersecting the world mask and shifting
    // its UV rectangle is exactly the source Graphics mask without another render target.
    renderer.draw(
        texture,
        Quad {
            source: [
                (column as f64 * 200.0 + left - position.x) as f32,
                (row as f64 * 200.0 + top - position.y) as f32,
                width as f32,
                height as f32,
            ],
            destination: camera.project(left, top, width, height),
            tint: tint(color, 1.0),
            blend: BlendMode::Normal,
        },
    );
}

fn tint(color: u32, alpha: f64) -> [f32; 4] {
    [
        ((color >> 16) & 255) as f32 / 255.0,
        ((color >> 8) & 255) as f32 / 255.0,
        (color & 255) as f32 / 255.0,
        alpha as f32,
    ]
}
