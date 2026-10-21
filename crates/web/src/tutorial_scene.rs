//! Tutorial UI and narration from the accepted world. The renderer's letterboxed
//! camera is projected once; no stage or dialogue clock lives in this adapter.

use solomon_sim::audio::{AudioCue, AudioOptions};
use solomon_sim::boneyard::BoneyardWorld;
use solomon_sim::boneyard::encounter::SolomonPhase;
use solomon_sim::boneyard::tutorial::{
    NativeTutorialBindingLabels, NativeTutorialCue, NativeTutorialInputMode, NativeTutorialSpeaker,
    native_tutorial_amulet_identity_matches, native_tutorial_instruction_baselines,
    native_tutorial_presentation,
};
use solomon_sim::economy::project_inventory_items;
use solomon_sim::loot::NativeLootDropKind;
use solomon_sim::player::PlayerState;
use solomon_sim::settings::{GameSettings, game_binding_label};
use solomon_sim::{HubWizard, Vec2};

use crate::audio::GameAudio;
use crate::courtyard::Camera;
use crate::tutorial::{
    TutorialNarrationView, TutorialPointerView, TutorialPreludeView, TutorialUi,
    TutorialViewSnapshot,
};

pub struct TutorialFrame<'a> {
    pub world: &'a BoneyardWorld,
    pub player: &'a PlayerState,
    pub wizard: &'a HubWizard,
    pub camera: &'a Camera,
    pub settings: &'a GameSettings,
    pub coarse_pointer: bool,
}

#[derive(Default)]
pub struct TutorialScene {
    run_id: Option<String>,
    narration_event_id: u64,
}

impl TutorialScene {
    /// Call after HUD and modal layout, even when a modal covers the world renderer.
    pub fn update(
        &mut self,
        ui: &mut TutorialUi,
        audio: &GameAudio,
        frame: Option<TutorialFrame<'_>>,
        time_ms: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        let frame = frame.filter(|frame| frame.world.tutorial.is_some());
        let run_id = frame.as_ref().map(|frame| frame.world.loaded.run_id.as_str());
        if self.run_id.as_deref() != run_id {
            stop_narration(audio)?;
            self.run_id = run_id.map(str::to_owned);
            self.narration_event_id = 0;
        }
        let Some(frame) = frame else {
            return ui.update(None, time_ms, width, height);
        };
        let state = frame.world.tutorial.as_ref().expect("Tutorial frame admitted above");
        if let Some(event) = &state.narration.current
            && event.event_id > self.narration_event_id
        {
            stop_narration(audio)?;
            let elapsed = event
                .cue
                .definition()
                .duration_ticks
                .saturating_sub(state.narration.ticks_remaining);
            audio.play(
                AudioCue::from_id(event.cue.id()).expect("stock Tutorial cue in audio catalog"),
                AudioOptions { offset_seconds: elapsed as f64 / 100.0, ..AudioOptions::default() },
            )?;
            self.narration_event_id = event.event_id;
        }

        let controls = &frame.settings.controls;
        let inventory = game_binding_label(&controls.open_inventory);
        let skills = game_binding_label(&controls.open_skills);
        let up = game_binding_label(&controls.move_up);
        let left = game_binding_label(&controls.move_left);
        let down = game_binding_label(&controls.move_down);
        let right = game_binding_label(&controls.move_right);
        let potion = game_binding_label(&controls.belt[3]);
        let secondary = game_binding_label(&controls.belt[0]);
        let presentation = native_tutorial_presentation(
            state,
            NativeTutorialBindingLabels {
                inventory: &inventory,
                move_down: &down,
                move_left: &left,
                move_right: &right,
                move_up: &up,
                potion: &potion,
                secondary: &secondary,
                skills: &skills,
            },
            if frame.coarse_pointer {
                NativeTutorialInputMode::Mobile
            } else {
                NativeTutorialInputMode::Desktop
            },
        );
        let size = Vec2::new(f64::from(width.max(1)), f64::from(height.max(1)));
        let layout = solomon_sim::viewport::game_viewport(size);
        let scale = layout.display_scale;
        let viewport = layout.logical_size;
        let baselines = native_tutorial_instruction_baselines(state.stage, viewport.y);
        let world_to_screen = |position: Vec2| {
            Vec2::new(
                (position.x - frame.camera.origin.x) * frame.camera.scale,
                (position.y - frame.camera.origin.y) * frame.camera.scale,
            )
        };
        let physical = |position: Vec2| Vec2::new(position.x * scale, position.y * scale);
        let world_target = matches!(state.stage.id(), 8 | 17)
            .then(|| {
                frame
                    .world
                    .loot
                    .actors
                    .iter()
                    .find(|actor| actor.kind == NativeLootDropKind::Sack)
                    .map(|actor| physical(world_to_screen(actor.position)))
            })
            .flatten();
        let solomon_pointer = if !state.intro_active
            && state.stage.id() <= 1
            && frame
                .world
                .encounter
                .as_ref()
                .is_some_and(|encounter| encounter.phase == SolomonPhase::Digging)
        {
            frame.world.loaded.scene.solomon_dig.as_ref().map(|dig| {
                let player = world_to_screen(frame.wizard.position);
                let target = world_to_screen(dig.position);
                TutorialPointerView {
                    origin: physical(tutorial_dig_pointer(player, target, viewport)),
                    target: physical(target),
                    scale,
                }
            })
        } else {
            None
        };
        let amulet = project_inventory_items(&frame.player.economy.backpack)
            .into_iter()
            .find(|entry| native_tutorial_amulet_identity_matches(entry.item))
            .map(|entry| entry.item.id);
        let view = TutorialViewSnapshot {
            stage: state.stage.id(),
            active: state.active,
            intro: state.intro_active.then_some(TutorialPreludeView {
                blend: state.intro_blend,
                fade: state.intro_fade,
            }),
            heading: presentation.heading.as_deref(),
            subheading: presentation.subheading.as_deref(),
            heading_baseline: baselines.map(|baselines| baselines.heading * scale),
            subheading_baseline: baselines
                .and_then(|baselines| baselines.subheading)
                .map(|value| value * scale),
            text_scale: scale,
            coarse_pointer: frame.coarse_pointer,
            selected_skill_hud_acknowledged: state.selected_skill_hud_acknowledged,
            resume_binding_label: if state.stage.id() == 10 { &inventory } else { &skills },
            amulet_item_id: amulet,
            narration: state.narration.current.as_ref().map(|event| TutorialNarrationView {
                event_id: event.event_id,
                speaker: match event.speaker {
                    NativeTutorialSpeaker::Sirmin => "Sirmin",
                    NativeTutorialSpeaker::Solomon => "Solomon Dark",
                },
                text: &event.text,
            }),
            world_target,
            solomon_pointer,
        };
        ui.update(Some(&view), time_ms, width, height)
    }
}

fn stop_narration(audio: &GameAudio) -> Result<(), String> {
    for cue in NativeTutorialCue::ALL {
        audio.stop_stream(
            AudioCue::from_id(cue.id()).expect("stock Tutorial cue in audio catalog"),
        )?;
    }
    Ok(())
}

/// `boneyardTutorialDigIndicatorLayout`: keep the edge arrow below the lesson text.
fn tutorial_dig_pointer(player: Vec2, target: Vec2, viewport: Vec2) -> Vec2 {
    let delta = Vec2::new(target.x - player.x, target.y - player.y);
    let distance = delta.x.hypot(delta.y);
    let direction = if distance == 0.0 {
        Vec2::new(1.0, 0.0)
    } else {
        Vec2::new(delta.x / distance, delta.y / distance)
    };
    let right = viewport.x - 64.0;
    let bottom = viewport.y - 120.0;
    if target.x >= 64.0 && target.x <= right && target.y >= 88.0 && target.y <= bottom {
        return Vec2::new(target.x - direction.x * 36.0, target.y - direction.y * 36.0);
    }
    let center = Vec2::new(viewport.x / 2.0, viewport.y / 2.0);
    let horizontal = if direction.x > 0.0 {
        (right - center.x) / direction.x
    } else if direction.x < 0.0 {
        (64.0 - center.x) / direction.x
    } else {
        f64::INFINITY
    };
    let vertical = if direction.y > 0.0 {
        (bottom - center.y) / direction.y
    } else if direction.y < 0.0 {
        (88.0 - center.y) / direction.y
    } else {
        f64::INFINITY
    };
    let distance = horizontal.min(vertical);
    Vec2::new(center.x + direction.x * distance, (center.y + direction.y * distance).max(230.0))
}
