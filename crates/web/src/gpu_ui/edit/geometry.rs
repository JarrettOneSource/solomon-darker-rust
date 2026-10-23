//! UTF-16 font geometry shared by actual GPU paint and pointer selection.

use egui::{Pos2, Rect, Vec2, pos2};

use super::model::Selection;

#[derive(Clone, Debug)]
pub struct Caret {
    pub index: usize,
    pub rect: Rect,
}

#[derive(Clone, Debug)]
pub struct Cluster {
    pub start: usize,
    pub end: usize,
    pub leading: Pos2,
    pub trailing: Pos2,
    pub rects: Vec<Rect>,
}

#[derive(Clone, Debug)]
pub struct PaintSpan {
    pub source: std::ops::Range<f32>,
    pub offset: f32,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub raster_text: String,
    pub paint_spans: Vec<PaintSpan>,
    pub top: f32,
    pub baseline: f32,
    pub height: f32,
    pub width: f32,
    pub carets: Vec<Caret>,
    pub clusters: Vec<Cluster>,
}

#[derive(Clone, Debug)]
pub struct Geometry {
    pub lines: Vec<Line>,
    pub size: Vec2,
}

impl Geometry {
    pub fn caret(&self, index: usize) -> Rect {
        let line = self
            .lines
            .iter()
            .find(|line| index >= line.start && index <= line.end)
            .unwrap_or_else(|| self.lines.last().expect("text has an empty row"));
        line.carets.iter().min_by_key(|caret| caret.index.abs_diff(index)).map_or_else(
            || Rect::from_min_size(pos2(0.0, line.top), Vec2::new(0.0, line.height)),
            |caret| caret.rect,
        )
    }

    pub fn hit(&self, point: Pos2) -> usize {
        self.hit_caret(point).index
    }

    pub fn hit_caret(&self, point: Pos2) -> Caret {
        let line = self
            .lines
            .iter()
            .min_by(|a, b| {
                let distance = |line: &Line| {
                    if point.y < line.top {
                        line.top - point.y
                    } else if point.y > line.top + line.height {
                        point.y - line.top - line.height
                    } else {
                        0.0
                    }
                };
                distance(a).total_cmp(&distance(b))
            })
            .expect("text has an empty row");
        let mut best = (f32::INFINITY, line.start, 0.0);
        let containing = |cluster: &&Cluster| {
            cluster.rects.iter().any(|rect| point.x >= rect.min.x && point.x < rect.max.x)
        };
        let inside = line.clusters.iter().any(|cluster| containing(&cluster));
        for cluster in line.clusters.iter().filter(|cluster| !inside || containing(cluster)) {
            for (index, edge) in [(cluster.start, cluster.leading), (cluster.end, cluster.trailing)]
            {
                let distance = (point.x - edge.x).abs();
                if distance < best.0 {
                    best = (distance, index, edge.x);
                }
            }
        }
        Caret {
            index: best.1,
            rect: Rect::from_min_size(pos2(best.2, line.top), Vec2::new(0.0, line.height)),
        }
    }

    pub fn selection_rects(&self, selection: Selection) -> Vec<Rect> {
        let range = selection.range();
        if range.is_empty() {
            return Vec::new();
        }
        let mut rectangles = Vec::new();
        for line in &self.lines {
            let mut row = Vec::new();
            for cluster in &line.clusters {
                if cluster.start < range.end && cluster.end > range.start {
                    row.extend(cluster.rects.iter().copied());
                }
            }
            row.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
            for rect in row {
                if let Some(last) = rectangles.last_mut() {
                    let last: &mut Rect = last;
                    if (last.min.y - rect.min.y).abs() < 0.01 && rect.min.x <= last.max.x + 0.02 {
                        *last = last.union(rect);
                        continue;
                    }
                }
                rectangles.push(rect);
            }
        }
        rectangles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_bidi_keeps_disjoint_selection_and_native_nonmonotonic_carets() {
        // Logical abאב, with the Hebrew letters painted in reverse visual order.
        let positions = [(0.0, 10.0), (10.0, 20.0), (40.0, 30.0), (30.0, 20.0)];
        let clusters: Vec<_> = positions
            .into_iter()
            .enumerate()
            .map(|(i, (start, end))| Cluster {
                start: i,
                end: i + 1,
                leading: pos2(start, 0.0),
                trailing: pos2(end, 0.0),
                rects: vec![Rect::from_min_max(
                    pos2(start.min(end), 0.0),
                    pos2(start.max(end), 20.0),
                )],
            })
            .collect();
        let geometry = Geometry {
            size: Vec2::new(40.0, 20.0),
            lines: vec![Line {
                start: 0,
                end: 4,
                text: "abאב".into(),
                raster_text: "abאב".into(),
                paint_spans: vec![PaintSpan {
                    source: f32::NEG_INFINITY..f32::INFINITY,
                    offset: 0.0,
                }],
                top: 0.0,
                baseline: 16.0,
                height: 20.0,
                width: 40.0,
                carets: vec![
                    Caret {
                        index: 0,
                        rect: Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(0.0, 20.0)),
                    },
                    Caret {
                        index: 3,
                        rect: Rect::from_min_size(pos2(30.0, 0.0), Vec2::new(0.0, 20.0)),
                    },
                    Caret {
                        index: 4,
                        rect: Rect::from_min_size(pos2(20.0, 0.0), Vec2::new(0.0, 20.0)),
                    },
                ],
                clusters,
            }],
        };
        assert_eq!(
            geometry.selection_rects(Selection { anchor: 1, focus: 3 }),
            vec![
                Rect::from_min_max(pos2(10.0, 0.0), pos2(20.0, 20.0)),
                Rect::from_min_max(pos2(30.0, 0.0), pos2(40.0, 20.0)),
            ]
        );
        assert_eq!(geometry.caret(3).min.x, 30.0);
        assert_eq!(geometry.caret(4).min.x, 20.0);
        assert_eq!(geometry.hit(pos2(39.0, 10.0)), 2);
        assert_eq!(geometry.hit(pos2(21.0, 10.0)), 4);
    }
}
