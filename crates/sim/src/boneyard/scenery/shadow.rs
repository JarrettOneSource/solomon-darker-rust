//! Native class auxiliaries from `native-scenery-shadow.ts` and
//! `native-scrub-shadow.ts`: actual glyph masks rather than guessed silhouettes.

use crate::boneyard::{
    BoneyardScene, shadows::NativeShadowRecord, terrain_surface::NATIVE_GROUND_PI,
};
use crate::float::{atan2, cos, fround as f, sin};
use crate::{NativeRng, Vec2};

use super::fence::{stored_inset, stored_length, stored_unit};
use super::{NativeSceneryLayer, NativeSceneryPlan, NativeScenerySource};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSceneryShadowGlyph {
    pub width: f64,
    pub height: f64,
    pub anchor_x: f64,
    pub anchor_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSceneryShadowTexture {
    Glyph(usize),
    FenceGrate,
    White,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSceneryShadowRole {
    TreeRootMask,
    SceneryFlatShadow,
    FenceFlatShadow,
    WallBaseShadow,
    ScrubFlatShadow,
    ScrubDirectionalShadow,
    ScrubSurfaceReflection,
}
impl NativeSceneryShadowRole {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TreeRootMask => "tree-root-mask",
            Self::SceneryFlatShadow => "scenery-flat-shadow",
            Self::FenceFlatShadow => "fence-flat-shadow",
            Self::WallBaseShadow => "wall-base-shadow",
            Self::ScrubFlatShadow => "scrub-flat-shadow",
            Self::ScrubDirectionalShadow => "scrub-directional-shadow",
            Self::ScrubSurfaceReflection => "scrub-surface-reflection",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSceneryShadowQuad {
    pub alphas: [f64; 4],
    pub role: NativeSceneryShadowRole,
    pub texture: NativeSceneryShadowTexture,
    pub tint: u32,
    pub uvs: [Vec2; 4],
    pub vertices: [Vec2; 4],
}
const FULL_UVS: [Vec2; 4] =
    [Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0)];

fn glyph_vertices(glyph: NativeSceneryShadowGlyph) -> [Vec2; 4] {
    [
        Vec2::new(-glyph.anchor_x, -glyph.anchor_y),
        Vec2::new(glyph.width - glyph.anchor_x, -glyph.anchor_y),
        Vec2::new(-glyph.anchor_x, glyph.height - glyph.anchor_y),
        Vec2::new(glyph.width - glyph.anchor_x, glyph.height - glyph.anchor_y),
    ]
}

#[must_use]
pub fn native_flat_glyph_shadow_vertices(glyph: NativeSceneryShadowGlyph) -> [Vec2; 4] {
    let mut vertices = glyph_vertices(glyph);
    let x = f(glyph.height * 0.25);
    let y = f(x * 1.25);
    for vertex in &mut vertices[..2] {
        *vertex = Vec2::new(f(vertex.x + x), f(vertex.y + y));
    }
    vertices
}

fn glyph_quad(
    entry: usize,
    position: Vec2,
    root_mask: bool,
    glyph: &dyn Fn(usize) -> Option<NativeSceneryShadowGlyph>,
) -> Option<NativeSceneryShadowQuad> {
    let glyph = glyph(entry)?;
    let vertices =
        if root_mask { glyph_vertices(glyph) } else { native_flat_glyph_shadow_vertices(glyph) };
    Some(NativeSceneryShadowQuad {
        alphas: [1.0; 4],
        role: if root_mask {
            NativeSceneryShadowRole::TreeRootMask
        } else {
            NativeSceneryShadowRole::SceneryFlatShadow
        },
        texture: NativeSceneryShadowTexture::Glyph(entry),
        tint: if root_mask { 0xffffff } else { 0 },
        uvs: FULL_UVS,
        vertices: vertices.map(|point| Vec2::new(f(point.x + position.x), f(point.y + position.y))),
    })
}

/// Persistent class auxiliaries have the main owner's slot. Flat Tree art fades
/// with its material; the independently owned Tree root mask remains full alpha.
#[must_use]
pub fn native_scenery_shadow_plan(
    scene: &BoneyardScene,
    plan: &NativeSceneryPlan,
    layer: &NativeSceneryLayer,
    complex_shadows: bool,
    glyph: &dyn Fn(usize) -> Option<NativeSceneryShadowGlyph>,
) -> Vec<NativeSceneryShadowQuad> {
    match layer.source {
        NativeScenerySource::FencePost { index, .. } => {
            if complex_shadows {
                return Vec::new();
            }
            let post = &plan.posts[index];
            let alternate = post.style == 1;
            assert!(
                post.variant < if alternate { 28 } else { 7 },
                "unsupported native Fencepost shadow selector"
            );
            glyph_quad(
                (if alternate { 320 } else { 36 }) + post.variant as usize,
                layer.position,
                false,
                glyph,
            )
            .into_iter()
            .collect()
        }
        NativeScenerySource::FenceBody { index, .. } => {
            if complex_shadows {
                return Vec::new();
            }
            let fence = &scene.fences[index];
            let (Some(&a), Some(&b)) = (fence.points.first(), fence.points.get(1)) else {
                return Vec::new();
            };
            if a == b {
                return Vec::new();
            }
            match fence.code() {
                0 => {
                    let [start, end] = stored_inset(a, b, 12.0);
                    vec![fence_quad(
                        [
                            Vec2::new(start.x, f(start.y - 52.0)),
                            Vec2::new(end.x, f(end.y - 52.0)),
                            start,
                            end,
                        ],
                        stored_length(Vec2::new(f(end.x - start.x), f(end.y - start.y))),
                    )]
                }
                4 => {
                    let [start, end] = stored_inset(a, b, 4.0);
                    let delta = Vec2::new(f(end.x - start.x), f(end.y - start.y));
                    let unit = stored_unit(delta);
                    let mut angle = f(f(atan2(unit.x, -unit.y)) * 180.0 / NATIVE_GROUND_PI);
                    if angle < 0.0 {
                        angle = f(angle + 360.0);
                    }
                    let radians = f(NATIVE_GROUND_PI * f(angle + 90.0) / 180.0);
                    let nx = f(f(sin(radians)) * 3.0);
                    let ny = f(-f(cos(radians)) * 3.0);
                    let top = |p: Vec2| Vec2::new(f(p.x - nx), f(f(p.y - 14.0) - ny));
                    let bottom = |p: Vec2| Vec2::new(f(p.x + nx), f(p.y + ny));
                    let mut vertices = [top(start), top(end), bottom(start), bottom(end)];
                    if vertices[2].y <= vertices[0].y {
                        vertices = [vertices[2], vertices[3], vertices[0], vertices[1]];
                    }
                    vec![fence_quad(vertices, stored_length(delta))]
                }
                _ => Vec::new(),
            }
        }
        NativeScenerySource::Object { index, entry } => {
            let object = &scene.objects[index];
            if object.type_id == 2001 {
                let variant = object.variant.map_or_else(
                    || entry.checked_sub(264).expect("Tree shadow native bank"),
                    |n| n as usize,
                );
                assert!(variant < 15, "Tree shadow requires materialized variant0..14");
                let mut output: Vec<_> =
                    glyph_quad(228 + variant, layer.position, true, glyph).into_iter().collect();
                if !complex_shadows {
                    output.extend(glyph_quad(264 + variant, layer.position, false, glyph));
                    if variant < 6 && object.secondary_visible != Some(false) {
                        let secondary = object.secondary_variant.unwrap_or(0) as usize;
                        assert!(
                            secondary < 21,
                            "unsupported native Tree secondary shadow selector"
                        );
                        output.extend(glyph_quad(
                            243 + secondary,
                            Vec2::new(layer.position.x, f(layer.position.y + 25.0)),
                            false,
                            glyph,
                        ));
                    }
                }
                return output;
            }
            if complex_shadows {
                return Vec::new();
            }
            let (first, count) = match object.type_id {
                2029 => (97, 17),
                2009 => (156, 21),
                2040 => (148, 4),
                _ => return Vec::new(),
            };
            let variant = object.variant.map_or_else(
                || entry.checked_sub(first).expect("native scenery shadow bank"),
                |n| n as usize,
            );
            assert!(variant < count, "unsupported native scenery shadow selector");
            glyph_quad(first + variant, layer.position, false, glyph).into_iter().collect()
        }
    }
}

#[must_use]
pub fn native_goodie_shadow_plan(
    position: Vec2,
    subtype: u32,
    phase: usize,
    complex_shadows: bool,
    glyph: &dyn Fn(usize) -> Option<NativeSceneryShadowGlyph>,
) -> Vec<NativeSceneryShadowQuad> {
    assert_eq!(subtype, 0, "unsupported native Goodie shadow subtype");
    if complex_shadows {
        Vec::new()
    } else {
        glyph_quad(145 + phase, position, false, glyph).into_iter().collect()
    }
}

#[must_use]
pub fn native_wall_skirt_plan(start: Vec2, end: Vec2) -> NativeSceneryShadowQuad {
    NativeSceneryShadowQuad {
        alphas: [1.0, 1.0, 0.0, 0.0],
        role: NativeSceneryShadowRole::WallBaseShadow,
        texture: NativeSceneryShadowTexture::White,
        tint: 0,
        uvs: FULL_UVS,
        vertices: [
            start,
            end,
            Vec2::new(start.x, f(start.y + 20.0)),
            Vec2::new(end.x, f(end.y + 20.0)),
        ],
    }
}

fn fence_quad(vertices: [Vec2; 4], length: f64) -> NativeSceneryShadowQuad {
    let span = f(length / 53.33333121405716);
    let mut points = vertices;
    for point in &mut points[..2] {
        *point = Vec2::new(f(point.x + 10.0), f(point.y + 10.0));
    }
    NativeSceneryShadowQuad {
        alphas: [0.5; 4],
        role: NativeSceneryShadowRole::FenceFlatShadow,
        texture: NativeSceneryShadowTexture::FenceGrate,
        tint: 0,
        uvs: [Vec2::ZERO, Vec2::new(span, 0.0), Vec2::new(0.0, 1.0), Vec2::new(span, 1.0)],
        vertices: points,
    }
}

/// Scrub's private native clock is renderer-owned and never enters saved authority.
pub struct NativeScrubShadowState {
    rng: NativeRng,
    last_tick: u64,
    pub phase: i32,
    pub uv_inset: f64,
    pub special_surface: bool,
}
impl NativeScrubShadowState {
    #[must_use]
    pub fn new(seed: usize, initial_tick: u64) -> Self {
        let mut rng = NativeRng::new(seed as i64);
        let phase = rng.draw_integer(360);
        Self { rng, last_tick: initial_tick, phase, uv_inset: 0.0, special_surface: false }
    }
    pub fn advance_to(&mut self, tick: f64) {
        while (self.last_tick as f64) < tick {
            self.phase = self.phase.wrapping_add(self.rng.draw_integer(3));
            self.last_tick += 1;
        }
    }
}

#[must_use]
#[allow(clippy::too_many_arguments)] // Source inputs plus borrowed glyph records keep asset I/O outside the kernel.
pub fn native_scrub_shadow_plan(
    entry: usize,
    glyph: NativeSceneryShadowGlyph,
    position: Vec2,
    state: &mut NativeScrubShadowState,
    records: &[NativeShadowRecord],
    complex_shadows: bool,
    special_surface_at: &dyn Fn(Vec2) -> bool,
    reflection: NativeSceneryShadowGlyph,
) -> Vec<NativeSceneryShadowQuad> {
    if !complex_shadows {
        return vec![NativeSceneryShadowQuad {
            alphas: [1.0; 4],
            role: NativeSceneryShadowRole::ScrubFlatShadow,
            texture: NativeSceneryShadowTexture::Glyph(entry),
            tint: 0,
            uvs: FULL_UVS,
            vertices: native_flat_glyph_shadow_vertices(glyph),
        }];
    }
    if state.uv_inset == 0.0 {
        state.uv_inset = f(0.2);
        let x = f(position.x);
        let y = f(position.y);
        // All three real calls occur even if the first one admits the surface.
        let center = special_surface_at(Vec2::new(x, y));
        let below = special_surface_at(Vec2::new(x, f(y + 10.0)));
        let above = special_surface_at(Vec2::new(x, f(y - 10.0)));
        state.special_surface = center || below || above;
    }
    let mut output = Vec::new();
    if state.special_surface {
        let wave = f(sin(f(f64::from(state.phase) / 3.0))).abs();
        let scale = f(wave * 0.25 + f(0.6));
        let alpha = f(0.35);
        output.push(NativeSceneryShadowQuad {
            alphas: [alpha; 4],
            role: NativeSceneryShadowRole::ScrubSurfaceReflection,
            texture: NativeSceneryShadowTexture::Glyph(21),
            tint: 0xffffff,
            uvs: FULL_UVS,
            vertices: glyph_vertices(reflection)
                .map(|point| Vec2::new(f(point.x * scale), f(point.y * scale))),
        });
    }
    let corners = glyph_vertices(glyph);
    for record in records {
        let direction = record.direction;
        let radians = f(atan2(direction.x, -direction.y));
        let mut degrees = f(radians * 180.0 / NATIVE_GROUND_PI);
        if degrees < 0.0 {
            degrees = f(degrees + 360.0);
        }
        let angle = f(-degrees * NATIVE_GROUND_PI / 180.0);
        let cosine = f(cos(angle));
        let sine = -f(sin(angle));
        let scale_x = f(10.0 * record.distance_fraction).min(1.0);
        let scale_y = f(0.8);
        let xx = f(cosine * scale_x);
        let yx = f(scale_y * f(sine * scale_x));
        let yy = f(scale_y * cosine);
        let near = |point: Vec2| {
            Vec2::new(
                f(f(point.x * xx - point.y * sine) + f(20.0 * direction.x)),
                f(f(point.x * yx + point.y * yy) + f(20.0 * direction.y)),
            )
        };
        let length = f(record.distance_fraction * 10.0 * record.distance_fraction);
        let far = |point: Vec2| {
            Vec2::new(
                f(point.x + f(length * f(record.projection_distance * direction.x))),
                f(point.y + f(length * f(record.projection_distance * direction.y))),
            )
        };
        let left = near(corners[2]);
        let right = near(corners[3]);
        let bottom = f(1.0 - state.uv_inset);
        output.push(NativeSceneryShadowQuad {
            alphas: [0.0, 0.0, 1.0, 1.0],
            role: NativeSceneryShadowRole::ScrubDirectionalShadow,
            texture: NativeSceneryShadowTexture::Glyph(entry),
            tint: 0,
            uvs: [FULL_UVS[0], FULL_UVS[1], Vec2::new(0.0, bottom), Vec2::new(1.0, bottom)],
            vertices: [far(left), far(right), left, right],
        });
    }
    output
}
