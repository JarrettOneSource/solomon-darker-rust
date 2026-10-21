//! Registered atlas uploads and sprite transforms shared by all enemy families.

use solomon_assets::{SpriteRecord, parse_bundle};
use solomon_sim::Vec2;
use solomon_sim::enemies::actor_presentation::{
    EnemyArtAtlas, EnemyArtBlend, EnemyArtPoints, EnemyArtRecord, EnemyArtRecords, EnemySpriteLayer,
};
use solomon_sim::float;
use solomon_sim::float::hypot;

use super::{EnemyDraw, EnemyDrawGroup, EnemyView};
use crate::assets::{fetch_bytes, load_image};
use crate::courtyard::Camera;
use crate::render::{BlendMode, Renderer, Texture, TextureColorMode, Vertex};

struct EnemyAtlas {
    records: Vec<SpriteRecord>,
    points: Vec<Vec<Vec2>>,
    texture: Texture,
}

impl EnemyAtlas {
    fn new(records: Vec<SpriteRecord>, texture: Texture) -> Self {
        let mut points = Vec::new();
        for record in &records {
            points.push(
                record
                    .points
                    .iter()
                    .map(|point| Vec2::new(f64::from(point[0]), f64::from(point[1])))
                    .collect(),
            );
        }
        Self { records, points, texture }
    }
}

pub(super) struct EnemyAtlases {
    banks: [EnemyAtlas; 6],
}

impl EnemyAtlases {
    pub(super) async fn load(
        renderer: &Renderer,
        bad_guys_records: Vec<SpriteRecord>,
        bad_guys_texture: Texture,
        dead_hawg_records: Vec<SpriteRecord>,
        dead_hawg_texture: Texture,
    ) -> Result<Self, String> {
        Ok(Self {
            banks: [
                EnemyAtlas::new(bad_guys_records, bad_guys_texture),
                EnemyAtlas::new(dead_hawg_records, dead_hawg_texture),
                load_atlas(renderer, "Demon").await?,
                load_atlas(renderer, "Faculty").await?,
                load_atlas(renderer, "Heartmonger").await?,
                load_atlas(renderer, "Unholy").await?,
            ],
        })
    }

    fn bank(&self, atlas: EnemyArtAtlas) -> &EnemyAtlas {
        &self.banks[match atlas {
            EnemyArtAtlas::BadGuys => 0,
            EnemyArtAtlas::DeadHawg => 1,
            EnemyArtAtlas::Demon => 2,
            EnemyArtAtlas::Faculty => 3,
            EnemyArtAtlas::Heartmonger => 4,
            EnemyArtAtlas::Unholy => 5,
        }]
    }

    pub(super) fn sprite(&self, atlas: EnemyArtAtlas, entry: usize) -> (&Texture, &SpriteRecord) {
        let bank = self.bank(atlas);
        let record = bank
            .records
            .get(entry)
            .unwrap_or_else(|| panic!("{} is missing stock record {entry}", atlas.id()));
        (&bank.texture, record)
    }
}

impl EnemyArtPoints for EnemyAtlases {
    fn points(&self, atlas: EnemyArtAtlas, entry: usize) -> &[Vec2] {
        &self.bank(atlas).points[entry]
    }
}

impl EnemyArtRecords for EnemyAtlases {
    fn record(&self, atlas: EnemyArtAtlas, entry: usize) -> EnemyArtRecord {
        let (_, record) = self.sprite(atlas, entry);
        EnemyArtRecord {
            anchor_x: f64::from(record.width) * 0.5 - f64::from(record.center_x),
            anchor_y: f64::from(record.height) * 0.5 - f64::from(record.center_y),
            width: f64::from(record.width),
            height: f64::from(record.height),
        }
    }
}

async fn load_atlas(renderer: &Renderer, name: &str) -> Result<EnemyAtlas, String> {
    let records = parse_bundle(&fetch_bytes(&format!("assets/stock/{name}.bundle")).await?)
        .map_err(|error| format!("{name}.bundle: {error}"))?;
    let image = load_image(&format!("assets/stock/{name}.png")).await?;
    Ok(EnemyAtlas::new(records, renderer.upload(&image)?))
}

impl EnemyView {
    pub(super) fn draw_group(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        group: &EnemyDrawGroup,
        region_tint: [f32; 3],
    ) {
        let region_tint = group.material_tint.map_or(region_tint, |tint| {
            let channels = color(tint, 1.0, [1.0; 3]);
            [channels[0], channels[1], channels[2]]
        });
        for draw in &group.draws {
            match draw {
                EnemyDraw::Sprite { layer, region_tint: lit } => self.draw_sprite(
                    renderer,
                    camera,
                    group,
                    layer,
                    if *lit { region_tint } else { [1.0; 3] },
                ),
                EnemyDraw::Underlay(layer) => {
                    let (texture, record) = self.art.sprite(layer.atlas, layer.entry);
                    let uv = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
                    let vertices = [0, 1, 2, 3].map(|index| {
                        let alpha = (layer.alphas[index].clamp(0.0, 1.0) * 255.0).trunc() / 255.0;
                        Vertex {
                            position: project(camera, group.position, layer.vertices[index]),
                            texel: [
                                record.x as f32 + uv[index][0] * record.width as f32,
                                record.y as f32 + uv[index][1] * record.height as f32,
                            ],
                            tint: color(layer.tint, alpha, [1.0; 3]),
                        }
                    });
                    // These trapezoids keep the native diagonal; changing it changes glyph UVs.
                    renderer.draw_triangles(
                        texture,
                        &[
                            [vertices[0], vertices[1], vertices[2]],
                            [vertices[1], vertices[3], vertices[2]],
                        ],
                        blend(layer.blend),
                    );
                }
                EnemyDraw::Segment(segment) => {
                    let start =
                        Vec2::new(segment.start.x * group.scale, segment.start.y * group.scale);
                    let end = Vec2::new(segment.end.x * group.scale, segment.end.y * group.scale);
                    let dx = end.x - start.x;
                    let dy = end.y - start.y;
                    let distance = hypot(dx, dy);
                    if distance == 0.0 {
                        continue;
                    }
                    let half = segment.width * group.scale * 0.5 / distance;
                    let normal = Vec2::new(-dy * half, dx * half);
                    let points = [
                        Vec2::new(start.x - normal.x, start.y - normal.y),
                        Vec2::new(start.x + normal.x, start.y + normal.y),
                        Vec2::new(end.x + normal.x, end.y + normal.y),
                        Vec2::new(end.x - normal.x, end.y - normal.y),
                    ];
                    draw_solid_quad(
                        renderer,
                        camera,
                        group.position,
                        points,
                        [color(segment.tint, segment.alpha, region_tint); 4],
                        blend(segment.blend),
                    );
                }
                EnemyDraw::Gradient(gradient) => {
                    let p = gradient.position;
                    let points = [
                        p,
                        Vec2::new(p.x + gradient.width, p.y),
                        Vec2::new(p.x + gradient.width, p.y + gradient.height),
                        Vec2::new(p.x, p.y + gradient.height),
                    ];
                    let first = color(gradient.start_color, gradient.start_alpha, [1.0; 3]);
                    let last = color(gradient.end_color, gradient.end_alpha, [1.0; 3]);
                    draw_solid_quad(
                        renderer,
                        camera,
                        group.position,
                        points,
                        [first, first, last, last],
                        BlendMode::Add,
                    );
                }
                EnemyDraw::AtlasMesh { .. } | EnemyDraw::Web(_) | EnemyDraw::MouthBeam { .. } => {
                    self.draw_mesh(renderer, camera, draw);
                }
            }
        }
    }

    fn draw_sprite(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        group: &EnemyDrawGroup,
        layer: &EnemySpriteLayer,
        region_tint: [f32; 3],
    ) {
        if layer.alpha <= 0.0 {
            return;
        }
        let (texture, record) = self.art.sprite(layer.atlas, layer.entry);
        let mut offset = layer.offset;
        let mut rotation = layer.rotation_radians;
        let mut scale_x = layer.scale_x.unwrap_or(layer.scale);
        let mut scale_y = layer.scale_y.unwrap_or(layer.scale);
        let (anchor_x, anchor_y) = if let Some(stretch) = layer.stretch {
            let dx = stretch.end.x - stretch.start.x;
            let dy = stretch.end.y - stretch.start.y;
            offset = Vec2::new(
                (stretch.start.x + stretch.end.x) * 0.5,
                (stretch.start.y + stretch.end.y) * 0.5,
            );
            rotation = dy.atan2(dx) - std::f64::consts::PI * 0.5;
            scale_x = layer.scale_x.unwrap_or(layer.scale);
            scale_y = hypot(dx, dy) / f64::from(record.height) * layer.scale_y.unwrap_or(1.0);
            (f64::from(record.width) * 0.5, f64::from(record.height) * 0.5)
        } else {
            (
                f64::from(record.width) * 0.5 - f64::from(record.center_x),
                f64::from(record.height) * 0.5 - f64::from(record.center_y),
            )
        };
        let width = f64::from(record.width);
        let height = f64::from(record.height);
        let positions = [(0.0, 0.0), (width, 0.0), (width, height), (0.0, height)].map(|(x, y)| {
            let x = (x - anchor_x) * scale_x;
            let y = (y - anchor_y) * scale_y;
            let local = Vec2::new(
                (offset.x + x * float::cos(rotation) - y * float::sin(rotation)) * group.scale,
                (offset.y + x * float::sin(rotation) + y * float::cos(rotation)) * group.scale,
            );
            project(camera, group.position, local)
        });
        renderer.draw_quad_with_color_mode(
            texture,
            source(record),
            positions,
            [color(layer.tint, layer.alpha, region_tint); 4],
            blend(layer.blend),
            if layer.texture_diffuse {
                TextureColorMode::AlphaMask
            } else {
                TextureColorMode::Texture
            },
        );
    }
}

pub(super) fn source(record: &SpriteRecord) -> [f32; 4] {
    [record.x as f32, record.y as f32, record.width as f32, record.height as f32]
}

pub(super) fn blend(mode: EnemyArtBlend) -> BlendMode {
    match mode {
        EnemyArtBlend::Normal => BlendMode::Normal,
        EnemyArtBlend::Add => BlendMode::Add,
    }
}

pub(super) fn color(tint: u32, alpha: f64, region: [f32; 3]) -> [f32; 4] {
    [
        ((tint >> 16) & 255) as f32 / 255.0 * region[0],
        ((tint >> 8) & 255) as f32 / 255.0 * region[1],
        (tint & 255) as f32 / 255.0 * region[2],
        alpha.clamp(0.0, 1.0) as f32,
    ]
}

pub(super) fn project(camera: &Camera, origin: Vec2, offset: Vec2) -> [f32; 2] {
    let point = camera.project(origin.x + offset.x, origin.y + offset.y, 0.0, 0.0);
    [point[0], point[1]]
}

fn draw_solid_quad(
    renderer: &mut Renderer,
    camera: &Camera,
    origin: Vec2,
    points: [Vec2; 4],
    colors: [[f32; 4]; 4],
    blend: BlendMode,
) {
    let vertices = [0, 1, 2, 3].map(|index| Vertex {
        position: project(camera, origin, points[index]),
        texel: [0.0, 0.0],
        tint: colors[index],
    });
    renderer.draw_solid_triangles(
        &[[vertices[0], vertices[1], vertices[2]], [vertices[0], vertices[2], vertices[3]]],
        blend,
    );
}
