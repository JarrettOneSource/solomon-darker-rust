//! The shared writer preserves source execution order and each world's old clear anchor.

use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::screen_flash::{NativeScreenFlashInput, NativeScreenFlashState};
use solomon_sim::secondary::NativeSecondaryScreenFlashState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Goldens {
    source_revision: String,
    frames: Vec<Frame>,
}

#[derive(Deserialize)]
struct Frame {
    tick: u64,
    #[serde(default)]
    reset: bool,
    writes: Vec<Write>,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Expected {
    epoch: u64,
    next_order: u64,
    writes: Vec<Write>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Write {
    #[serde(default)]
    order: u64,
    tick: u64,
    world_key: String,
    position: Point,
    flash: Flash,
    only_if_clear: bool,
}

#[derive(Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Flash {
    alpha: f64,
    blue: f64,
    decay_per_tick: f64,
    green: f64,
    point_attenuated: bool,
    red: f64,
}

impl Flash {
    fn state(&self) -> NativeSecondaryScreenFlashState {
        NativeSecondaryScreenFlashState {
            alpha: self.alpha,
            blue: self.blue,
            decay_per_tick: self.decay_per_tick,
            green: self.green,
            point_attenuated: self.point_attenuated,
            red: self.red,
        }
    }
}

#[test]
fn current_shared_screen_writes_keep_conditional_and_cross_world_order() {
    let fixture = include_str!("fixtures/website/current-screen-flash-goldens.json");
    let goldens: Goldens = serde_json::from_str(fixture).unwrap();
    assert_eq!(goldens.source_revision, "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    let mut state = NativeScreenFlashState::default();
    for frame in goldens.frames {
        if frame.reset {
            state.reset();
        }
        let mut writer = state.writer(frame.tick);
        for write in frame.writes {
            writer.write(NativeScreenFlashInput {
                tick: write.tick,
                world_key: write.world_key,
                position: Vec2::new(write.position.x, write.position.y),
                flash: write.flash.state(),
                only_if_clear: write.only_if_clear,
            });
        }
        state = writer.finish();
        let context = format!("tick{}", frame.tick);
        assert_eq!(state.epoch, frame.expected.epoch, "{context} epoch");
        assert_eq!(state.next_order, frame.expected.next_order, "{context} next order");
        assert_eq!(state.writes.len(), frame.expected.writes.len(), "{context} retained writes");
        for (actual, expected) in state.writes.iter().zip(&frame.expected.writes) {
            assert_eq!(actual.order, expected.order, "{context} write order");
            assert_eq!(actual.tick, expected.tick, "{context} write tick");
            assert_eq!(actual.world_key, expected.world_key, "{context} world");
            assert_eq!(
                actual.position,
                Vec2::new(expected.position.x, expected.position.y),
                "{context} position"
            );
            assert_eq!(actual.flash, expected.flash.state(), "{context} authored flash");
            assert_eq!(actual.only_if_clear, expected.only_if_clear, "{context} admission");
        }
    }
}
