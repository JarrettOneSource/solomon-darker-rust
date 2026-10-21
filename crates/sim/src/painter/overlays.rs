//! The final `promoteOwnerOverlays` pass runs after the native region planner.
//! Pixi sorts its child list in place, so equal depths retain the preceding frame's
//! display order, with newly created views appended in creation order.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct PainterOwnerOverlay {
    pub draw_id: usize,
    pub owner_draw_id: usize,
    pub view_id: u64,
}

#[derive(Clone, Copy, Debug)]
struct RetainedView {
    id: u64,
    creation_order: u64,
    depth: f64,
}

#[derive(Default)]
pub struct PainterOverlayOrder {
    views: Vec<RetainedView>,
}

impl PainterOverlayOrder {
    pub fn clear(&mut self) {
        self.views.clear();
    }

    /// `painters` carries the planner's actual display depths. Empty retained views
    /// keep their previous depth until they emit a painter again, as Pixi does.
    pub fn apply(
        &mut self,
        painters: &[(usize, f64)],
        overlays: &[PainterOwnerOverlay],
        live_views: &[(u64, u64)],
    ) -> Vec<usize> {
        let live: BTreeMap<_, _> = live_views.iter().copied().collect();
        self.views.retain(|view| live.get(&view.id) == Some(&view.creation_order));
        let mut admitted: Vec<_> = live_views
            .iter()
            .filter(|(id, _)| !self.views.iter().any(|view| view.id == *id))
            .map(|&(id, creation_order)| RetainedView { id, creation_order, depth: 0.0 })
            .collect();
        admitted.sort_by_key(|view| view.creation_order);
        self.views.extend(admitted);

        let mut depths: BTreeMap<_, _> = painters.iter().copied().collect();
        let mut overlay_views = BTreeMap::new();
        for overlay in overlays {
            let Some(&base_depth) = depths.get(&overlay.draw_id) else { continue };
            let depth = depths
                .get(&overlay.owner_draw_id)
                .map_or(base_depth, |owner_depth| base_depth.max(owner_depth + 0.5));
            depths.insert(overlay.draw_id, depth);
            let view = self
                .views
                .iter_mut()
                .find(|view| view.id == overlay.view_id)
                .expect("an owner overlay belongs to an admitted spell view");
            view.depth = depth;
            overlay_views.insert(overlay.draw_id, overlay.view_id);
        }
        self.views.sort_by(|left, right| left.depth.total_cmp(&right.depth));
        let retained_order: BTreeMap<_, _> =
            self.views.iter().enumerate().map(|(rank, view)| (view.id, rank)).collect();
        let mut order: Vec<_> = painters.iter().map(|&(id, _)| id).collect();
        order.sort_by(|left, right| {
            depths[left].total_cmp(&depths[right]).then_with(|| {
                match (overlay_views.get(left), overlay_views.get(right)) {
                    (Some(left), Some(right)) => retained_order[left].cmp(&retained_order[right]),
                    _ => std::cmp::Ordering::Equal,
                }
            })
        });
        order
    }
}
