//! Delivered Tree frames from `boneyard-tree-frame-history.ts`. Changing camera
//! ownership discards guessed history without resetting the Trees' retained fade.

use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::scenery::{
    NativeTreeActorPoses, NativeTreeOcclusionFrame, NativeTreeOcclusionPresentation,
    native_tree_actor_positions,
};
use solomon_sim::boneyard::world::boneyard_camera_origin;

use crate::presentation::NativePresentationTimeline;

#[derive(Clone, PartialEq)]
struct TreeCameraEpoch {
    run_id: String,
    focus_player_id: Option<String>,
    viewport: Vec2,
    zoom: f64,
    camera_limits: BoneyardBounds,
}

#[derive(Default)]
pub(super) struct TreeFrameHistory {
    epoch: Option<TreeCameraEpoch>,
    epoch_start_tick: u64,
    last_tick: u64,
}

pub(crate) struct TreeFrameSource<'a> {
    pub run_id: &'a str,
    pub local_player_id: &'a str,
    pub focus_player_id: Option<&'a str>,
    pub display_tick: f64,
    pub actor_poses: &'a NativeTreeActorPoses,
    pub camera_bounds: BoneyardBounds,
    pub camera_limits: BoneyardBounds,
    pub viewport: Vec2,
    pub zoom: f64,
    pub timeline: Option<&'a NativePresentationTimeline>,
}

impl TreeFrameHistory {
    pub(super) fn update(
        &mut self,
        trees: &mut NativeTreeOcclusionPresentation,
        source: &TreeFrameSource<'_>,
    ) {
        let tick = source.display_tick.floor() as u64;
        let epoch = TreeCameraEpoch {
            run_id: source.run_id.to_owned(),
            focus_player_id: source.focus_player_id.map(str::to_owned),
            viewport: source.viewport,
            zoom: source.zoom,
            camera_limits: source.camera_limits,
        };
        if self.epoch.as_ref() != Some(&epoch) || tick < self.last_tick {
            self.epoch = Some(epoch);
            self.epoch_start_tick = tick;
            self.last_tick = tick;
        }
        let start = self.epoch_start_tick.max(self.last_tick + 1);
        self.last_tick = tick;
        let current = NativeTreeOcclusionFrame {
            actor_positions: native_tree_actor_positions(
                source.actor_poses,
                source.local_player_id,
            ),
            camera_bounds: source.camera_bounds,
        };
        let (Some(timeline), Some(focus_id)) = (source.timeline, source.focus_player_id) else {
            trees.update(source.display_tick, &current, None, |_| None);
            return;
        };
        let mut frames = BTreeMap::new();
        let mut sample = |at: u64| {
            frames
                .entry(at)
                .or_insert_with(|| {
                    let actors = timeline.sample_scenery_actor_poses_at_tick(at as f64)?;
                    let focus = timeline.sample_player_position_at_tick(at as f64, focus_id)?;
                    let origin = boneyard_camera_origin(
                        focus,
                        source.camera_limits,
                        source.viewport,
                        source.zoom,
                    );
                    Some(NativeTreeOcclusionFrame {
                        actor_positions: native_tree_actor_positions(
                            &actors,
                            source.local_player_id,
                        ),
                        camera_bounds: BoneyardBounds::new(
                            origin.x,
                            origin.y,
                            source.viewport.x / source.zoom,
                            source.viewport.y / source.zoom,
                        ),
                    })
                })
                .clone()
        };
        let Some(current) = sample(tick) else {
            trees.update(source.display_tick, &current, None, |_| None);
            return;
        };
        let mut first = tick.min(start);
        let mut last = tick;
        // Delivered history is contiguous. Skip suspended-tab gaps without
        // inventing the camera or visiting every unavailable tick.
        while first < last {
            let middle = first + (last - first) / 2;
            if sample(middle).is_some() {
                last = middle;
            } else {
                first = middle + 1;
            }
        }
        trees.update(source.display_tick, &current, Some(first), sample);
    }
}
