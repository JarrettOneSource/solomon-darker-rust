//! The College teacher and its four release painters from `renderer/hub-world-scene.ts`.
//!
//! Images are the Website's stock-derived registered frames. Keeping those logical frames
//! preserves their centered anchors without copying the full enemy atlas into the browser.

use solomon_sim::Vec2;
use solomon_sim::hub::painter::{PainterLane, PainterLayer};
use solomon_sim::hub::teacher::{
    HUB_TEACHER_PAINTER_ORDINAL, HUB_TEACHER_POSITION, HUB_TEACHER_RUNE_ALPHA,
    HUB_TEACHER_RUNE_CENTER, HubTeacherBurstMember, HubTeacherDrawPlan,
};

use crate::assets;
use crate::courtyard::Camera;
use crate::render::{BlendMode, Quad, Renderer, Texture};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeacherPass {
    Actor,
    Column,
    Frames,
    Flare,
    Core,
}

pub struct TeacherView {
    actor: Texture,
    rune: Texture,
    shadow: Texture,
    column: Texture,
    frames: Texture,
    flare: Texture,
    core: Texture,
}

impl TeacherView {
    pub async fn load(renderer: &Renderer) -> Result<Self, String> {
        Ok(Self {
            actor: texture(renderer, "hub-teacher-frames").await?,
            rune: texture(renderer, "hub-teacher-rune").await?,
            shadow: texture(renderer, "player-shadow").await?,
            column: texture(renderer, "hub-teacher-burst-column").await?,
            frames: texture(renderer, "hub-teacher-burst-frames").await?,
            flare: texture(renderer, "hub-teacher-burst-flare").await?,
            core: texture(renderer, "hub-teacher-burst-core").await?,
        })
    }

    #[must_use]
    pub fn painters(plan: &HubTeacherDrawPlan) -> Vec<(PainterLayer, TeacherPass)> {
        let mut painters = vec![(
            PainterLayer {
                visible: true,
                is_insertion: false,
                manager_lane: PainterLane::Actor,
                registration_ordinal: HUB_TEACHER_PAINTER_ORDINAL,
                world_y: HUB_TEACHER_POSITION.y,
                sort_bias: 0.0,
            },
            TeacherPass::Actor,
        )];
        if let Some([column, frames]) = plan.world_painters {
            if plan.burst.column.visible {
                painters.push((column, TeacherPass::Column));
            }
            if plan.burst.frames.member.visible {
                painters.push((frames, TeacherPass::Frames));
            }
        }
        painters
    }

    pub fn draw(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        plan: &HubTeacherDrawPlan,
        pass: TeacherPass,
    ) {
        let burst = plan.burst;
        let (texture, frame, count, member, blend) = match pass {
            TeacherPass::Actor => {
                self.draw_actor(renderer, camera, plan.actor_frame);
                return;
            }
            TeacherPass::Column => (&self.column, 0, 1, burst.column, BlendMode::Normal),
            TeacherPass::Frames => {
                (&self.frames, burst.frames.frame, 11, burst.frames.member, BlendMode::Add)
            }
            TeacherPass::Flare => (&self.flare, 0, 1, burst.flare, BlendMode::Normal),
            TeacherPass::Core => (&self.core, 0, 1, burst.core, BlendMode::Normal),
        };
        if member.visible {
            draw_centered(
                renderer,
                camera,
                texture,
                strip_frame(texture, frame, count),
                plan.release_origin,
                member,
                blend,
            );
        }
    }

    fn draw_actor(&self, renderer: &mut Renderer, camera: &Camera, frame: u32) {
        let rune_center = Vec2::new(
            HUB_TEACHER_POSITION.x + HUB_TEACHER_RUNE_CENTER.x,
            HUB_TEACHER_POSITION.y + HUB_TEACHER_RUNE_CENTER.y,
        );
        let rune = sprite(HUB_TEACHER_RUNE_ALPHA, 1.0);
        draw_centered(
            renderer,
            camera,
            &self.rune,
            strip_frame(&self.rune, 0, 1),
            rune_center,
            rune,
            BlendMode::Normal,
        );
        let shadow = sprite(1.0, 1.25);
        draw_centered(
            renderer,
            camera,
            &self.shadow,
            strip_frame(&self.shadow, 0, 1),
            HUB_TEACHER_POSITION,
            shadow,
            BlendMode::Normal,
        );
        draw_centered(
            renderer,
            camera,
            &self.actor,
            strip_frame(&self.actor, frame, 4),
            HUB_TEACHER_POSITION,
            sprite(1.0, 1.0),
            BlendMode::Normal,
        );
    }
}

async fn texture(renderer: &Renderer, name: &str) -> Result<Texture, String> {
    let image = assets::load_image(&format!("assets/stock/{name}.png")).await?;
    renderer.upload(&image)
}

fn sprite(alpha: f64, scale: f64) -> HubTeacherBurstMember {
    HubTeacherBurstMember { alpha, scale_x: scale, scale_y: scale, visible: true }
}

fn strip_frame(texture: &Texture, frame: u32, count: u32) -> [f32; 4] {
    let width = texture.width / count as f32;
    [frame as f32 * width, 0.0, width, texture.height]
}

fn draw_centered(
    renderer: &mut Renderer,
    camera: &Camera,
    texture: &Texture,
    source: [f32; 4],
    position: Vec2,
    member: HubTeacherBurstMember,
    blend: BlendMode,
) {
    // A signed width mirrors the frame strip around its center, like Pixi scale.x < 0.
    let width = f64::from(source[2]) * member.scale_x;
    let height = f64::from(source[3]) * member.scale_y;
    renderer.draw(
        texture,
        Quad {
            source,
            destination: camera.project(
                position.x - width / 2.0,
                position.y - height / 2.0,
                width,
                height,
            ),
            tint: [1.0, 1.0, 1.0, member.alpha as f32],
            blend,
        },
    );
}
