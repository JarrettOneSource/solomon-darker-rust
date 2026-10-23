//! GPU groups for Source ModEnemies root filters and native text shadows.
//! Typed paint markers carry data only; no external callback code is executed.

use egui::{ClippedPrimitive, Rect, epaint::Primitive};
use solomon_graphics::{BlendMode, ColorFilter, ShadowOutput};

use super::ImageShadow;

#[derive(Clone, Debug)]
pub struct GroupEffect {
    pub opacity: f32,
    /// CSS ordering: the first shadow is nearest the foreground source.
    pub shadows: Vec<ImageShadow>,
}

impl Default for GroupEffect {
    fn default() -> Self {
        Self { opacity: 1.0, shadows: Vec::new() }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Style {
    pub effect: GroupEffect,
    pub filter: ColorFilter,
    pub blend: BlendMode,
    pub output: ShadowOutput,
}

pub(super) enum Boundary {
    Begin(Style),
    End,
}

pub(super) enum Node {
    Mesh { mesh: egui::Mesh, clip: Rect },
    Group { style: Style, clip: Rect, children: Vec<Node> },
}

pub(super) fn parse(primitives: Vec<ClippedPrimitive>) -> Result<Vec<Node>, String> {
    collect(&mut primitives.into_iter(), false)
}

fn collect(
    primitives: &mut impl Iterator<Item = ClippedPrimitive>,
    in_group: bool,
) -> Result<Vec<Node>, String> {
    let mut nodes = Vec::new();
    while let Some(primitive) = primitives.next() {
        match primitive.primitive {
            Primitive::Mesh(mesh) => {
                if !mesh.is_valid() {
                    return Err("GPU UI produced an invalid mesh".to_owned());
                }
                nodes.push(Node::Mesh { mesh, clip: primitive.clip_rect });
            }
            Primitive::Callback(callback) => {
                let boundary = callback
                    .callback
                    .downcast_ref::<Boundary>()
                    .ok_or("GPU UI does not admit external paint callbacks")?;
                match boundary {
                    Boundary::Begin(style) => nodes.push(Node::Group {
                        style: style.clone(),
                        clip: primitive.clip_rect,
                        children: collect(primitives, true)?,
                    }),
                    Boundary::End if in_group => return Ok(nodes),
                    Boundary::End => {
                        return Err("GPU UI group ended without a beginning".to_owned());
                    }
                }
            }
        }
    }
    if in_group { Err("GPU UI group has no ending".to_owned()) } else { Ok(nodes) }
}

pub(super) fn content_bounds(nodes: &[Node]) -> Rect {
    nodes.iter().fold(Rect::NOTHING, |bounds, node| {
        let painted = match node {
            Node::Mesh { mesh, clip } => mesh.calc_bounds().intersect(*clip),
            Node::Group { style, clip, children } => {
                effect_bounds(content_bounds(children), &style.effect).intersect(*clip)
            }
        };
        if painted.is_positive() { bounds.union(painted) } else { bounds }
    })
}

pub(super) fn effect_bounds(source: Rect, effect: &GroupEffect) -> Rect {
    if !source.is_positive() {
        return Rect::NOTHING;
    }
    effect.shadows.iter().fold(source, |bounds, shadow| {
        bounds.union(source.translate(shadow.offset).expand(shadow.blur * 3.0))
    })
}

/// Source pixels farther away cannot contribute to the finite shadow footprint.
pub(super) fn capture_clip(clip: Rect, effect: &GroupEffect) -> Rect {
    effect.shadows.iter().fold(clip, |bounds, shadow| {
        bounds.union(clip.translate(-shadow.offset).expand(shadow.blur * 3.0))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, TextureId, epaint::PaintCallback, pos2, vec2};
    use std::sync::Arc;

    fn marker(boundary: Boundary) -> ClippedPrimitive {
        ClippedPrimitive {
            clip_rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)),
            primitive: Primitive::Callback(PaintCallback {
                rect: Rect::EVERYTHING,
                callback: Arc::new(boundary),
            }),
        }
    }
    fn style() -> Style {
        Style {
            effect: GroupEffect {
                opacity: 0.75,
                shadows: vec![ImageShadow {
                    offset: vec2(0.0, 1.0),
                    blur: 2.0,
                    color: Color32::BLACK,
                }],
            },
            filter: ColorFilter::default(),
            blend: BlendMode::Normal,
            output: ShadowOutput::Source,
        }
    }
    fn mesh() -> ClippedPrimitive {
        let mut mesh = egui::Mesh::with_texture(TextureId::Managed(0));
        mesh.add_rect_with_uv(
            Rect::from_min_max(pos2(20.0, 30.0), pos2(40.0, 50.0)),
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        ClippedPrimitive { clip_rect: Rect::EVERYTHING, primitive: Primitive::Mesh(mesh) }
    }
    #[test]
    fn actual_gpu_group_keeps_its_content_order_and_filter_footprint() {
        let nodes = parse(vec![
            marker(Boundary::Begin(style())),
            mesh(),
            marker(Boundary::Begin(style())),
            mesh(),
            marker(Boundary::End),
            marker(Boundary::End),
        ])
        .unwrap();
        assert_eq!(nodes.len(), 1);
        let Node::Group { children, style, .. } = &nodes[0] else { panic!("outer group") };
        assert_eq!(children.len(), 2);
        assert!(matches!(children[0], Node::Mesh { .. }));
        assert!(matches!(children[1], Node::Group { .. }));
        assert_eq!(style.effect.opacity, 0.75);
        assert_eq!(
            effect_bounds(content_bounds(&parse(vec![mesh()]).unwrap()), &style.effect),
            Rect::from_min_max(pos2(14.0, 25.0), pos2(46.0, 57.0))
        );
        assert_eq!(
            capture_clip(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)), &style.effect),
            Rect::from_min_max(pos2(-6.0, -7.0), pos2(106.0, 105.0))
        );
    }
    #[test]
    fn incomplete_or_external_group_callbacks_are_rejected_before_gpu_submission() {
        assert!(parse(vec![marker(Boundary::End)]).is_err());
        assert!(parse(vec![marker(Boundary::Begin(style())), mesh()]).is_err());
        assert!(
            parse(vec![ClippedPrimitive {
                clip_rect: Rect::EVERYTHING,
                primitive: Primitive::Callback(PaintCallback {
                    rect: Rect::EVERYTHING,
                    callback: Arc::new(7_u32)
                })
            }])
            .is_err()
        );
    }
}
