//! Browser client: loads the stock atlases, runs the Hub simulation at 100 Hz, and draws the
//! courtyard with WebGL2.
//!
//! The simulation lives in `solomon-sim` and never touches the browser; this crate owns asset
//! loading, input, the fixed-tick loop, and drawing.

mod assets;
pub mod audio;
pub mod battle;
mod boneyard;
pub mod boneyard_weather;
pub mod cheats;
pub mod college_intro;
pub mod compact_masks;
mod courtyard;
pub mod deployment;
pub mod desktop;
pub mod enemies;
pub mod fullscreen;
mod game;
pub mod game_over;
pub mod hosted;
pub mod hotbar;
pub mod hud;
pub mod input;
pub mod loot;
pub mod match_loading;
pub mod menu;
mod mobile_editor;
pub mod mobile_layout;
pub mod mod_audio;
pub mod mod_consumable_effects;
pub mod mod_content;
pub mod mod_content_projection;
pub mod mod_minimap;
pub mod mod_panels;
pub mod mod_presentation_assets;
pub mod mod_skill_quickbar;
pub mod mod_skills;
pub mod mod_world_overlays;
mod native_mesh;
pub mod navigation_data;
pub mod network;
mod npc;
pub mod observer;
pub mod player_effects;
pub mod player_panels;
pub mod presentation;
pub mod render;
mod rooms;
pub mod secondary;
#[cfg(feature = "browser-fixtures")]
mod secondary_fixture;
pub mod shared_gameplay;
mod skill_picker;
pub mod social;
pub mod solomon;
mod spectator;
pub mod spells;
mod storage;
mod teacher;
pub mod tutorial;
pub mod tutorial_scene;
pub mod ui;
pub mod wizard;
pub mod world_overlays;
pub mod world_screen_feedback;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlElement, Window};

use crate::boneyard::PreparedBoneyardView;
use crate::game::Game;

thread_local! {
    static GAME: RefCell<Option<Rc<RefCell<Game>>>> = const { RefCell::new(None) };
}

/// Browser journey snapshot: x, y, heading, gait, walk phase, tick, region, transition, alpha.
#[wasm_bindgen]
pub fn wizard_state() -> Vec<f64> {
    GAME.with(|slot| {
        slot.borrow().as_ref().map_or_else(Vec::new, |game| game.borrow().wizard_snapshot())
    })
}

/// Student count followed by id/x/y triples for the live population.
#[wasm_bindgen]
pub fn npc_state() -> Vec<f64> {
    GAME.with(|slot| {
        slot.borrow().as_ref().map_or_else(Vec::new, |game| game.borrow().npc_snapshot())
    })
}

/// One sampled input frame, including retained world aim and one-shot belt press count.
#[wasm_bindgen]
pub fn gameplay_input_state() -> Vec<f64> {
    GAME.with(|slot| {
        slot.borrow().as_ref().map_or_else(Vec::new, |game| game.borrow().gameplay_input_snapshot())
    })
}

/// Exercises the same offline map loading and installation used by the active session.
#[wasm_bindgen]
pub async fn enter_boneyard(seed_index: u32) -> Result<(), JsValue> {
    let game = GAME.with(|slot| slot.borrow().clone()).ok_or("the game has not started")?;
    let mut seed = [0; 16];
    seed[..4].copy_from_slice(&seed_index.to_be_bytes());
    let loaded = solomon_assets::boneyard::load_boneyard(seed).map_err(JsValue::from)?;
    set_status("Loading Boneyard…");
    let prepared = PreparedBoneyardView::load(&loaded).await.map_err(JsValue::from)?;
    let navigation = navigation_data::load_navigation(&loaded).await.map_err(JsValue::from)?;
    game.borrow_mut().install_boneyard(loaded, prepared, navigation).map_err(JsValue::from)?;
    set_status("");
    Ok(())
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(message) = run().await {
            web_sys::console::error_1(&JsValue::from_str(&message));
            if let Err(error) = game::editor_test::report_failure(&message) {
                web_sys::console::error_1(&JsValue::from_str(&error));
            }
            if let Some(status) = status_element() {
                status.set_text_content(Some(&message));
                let _ = status.style().set_property("color", "#ff7b7b");
            }
        }
    });
}

fn status_element() -> Option<HtmlElement> {
    web_sys::window()?.document()?.get_element_by_id("status")?.dyn_into().ok()
}

fn set_status(text: &str) {
    if let Some(status) = status_element() {
        status.set_text_content(Some(text));
    }
}

async fn run() -> Result<(), String> {
    // Loading retains the world, the checkpoint, and pending GPU resources across awaits.
    // Keep that large future on the heap: nesting its unoptimized state in this poll frame
    // exhausted the wasm stack while decoding a saved PlayerState (save-resume journey).
    let game = Rc::new(RefCell::new(Box::pin(Game::load()).await?));
    let window = web_sys::window().ok_or("no window")?;
    GAME.with(|slot| *slot.borrow_mut() = Some(game.clone()));
    Game::attach_desktop(&game, &window)?;
    if !game.borrow().is_editor_test() {
        install_save_events(&window, &game)?;
    }
    schedule_frames(window, game);
    Ok(())
}

/// The editor's hidden-frame timer polls only admission and disposal, never gameplay.
#[wasm_bindgen]
pub fn editor_test_cleanup() -> bool {
    GAME.with(|slot| {
        let slot = slot.borrow();
        let Some(game) = slot.as_ref() else { return false };
        let mut game = game.borrow_mut();
        match game.editor_test_cleanup() {
            Ok(disposed) => disposed,
            Err(message) => {
                if let Err(error) = game.editor_test_failed(&message) {
                    web_sys::console::error_1(&JsValue::from_str(&error));
                }
                true
            }
        }
    })
}

/// The animation-frame closure, shared with itself so each frame can request the next one.
type FrameCallback = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

fn schedule_frames(window: Window, game: Rc<RefCell<Game>>) {
    let callback: FrameCallback = Rc::new(RefCell::new(None));
    let next = callback.clone();
    let requester = window.clone();
    *callback.borrow_mut() = Some(Closure::new(move |time: f64| {
        let result = game.borrow_mut().frame(time);
        if let Err(message) = result {
            web_sys::console::error_1(&JsValue::from_str(&message));
            set_status(&message);
            if let Err(error) = game.borrow_mut().editor_test_failed(&message) {
                web_sys::console::error_1(&JsValue::from_str(&error));
            }
            return;
        }
        if game.borrow().editor_test_disposed() {
            return;
        }
        write_pending_save(&game);
        run_hosted_work(&game);
        load_pending_world(&game);
        if let Some(closure) = next.borrow().as_ref() {
            request_frame(&requester, closure);
        }
    }));
    if let Some(closure) = callback.borrow().as_ref() {
        request_frame(&window, closure);
    }
}

fn load_pending_world(game: &Rc<RefCell<Game>>) {
    let Some((generation, loaded)) = game.borrow_mut().take_world_load() else { return };
    let game = game.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let result = async {
            let prepared = PreparedBoneyardView::load(&loaded).await?;
            let navigation = navigation_data::load_navigation(&loaded).await?;
            Ok::<_, String>((prepared, navigation))
        }
        .await;
        let mut game = game.borrow_mut();
        if !game.world_load_current(generation) {
            return;
        }
        let installed = result
            .and_then(|(prepared, navigation)| game.install_boneyard(loaded, prepared, navigation));
        match installed {
            Ok(()) => set_status(""),
            Err(error) => {
                game.world_load_failed(&error);
                if let Err(error) = game.editor_test_failed(&error) {
                    web_sys::console::error_1(&JsValue::from_str(&error));
                }
            }
        }
    });
}

fn write_pending_save(game: &Rc<RefCell<Game>>) {
    let pending = game.borrow_mut().take_save();
    if let Some(pending) = pending {
        let game = game.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = pending.write().await;
            game.borrow_mut().save_completed(result);
        });
    }
}

fn run_hosted_work(game: &Rc<RefCell<Game>>) {
    for pending in game.borrow_mut().take_hosted_work() {
        let game = game.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let completed = pending.execute().await;
            if let Err(error) = game.borrow_mut().complete_hosted_work(completed) {
                web_sys::console::error_1(&JsValue::from_str(&error));
                set_status(&error);
            }
        });
    }
}

fn install_save_events(window: &Window, game: &Rc<RefCell<Game>>) -> Result<(), String> {
    let document = window.document().ok_or("no document for saved-game events")?;
    for (target, name) in [
        (document.clone().unchecked_into::<web_sys::EventTarget>(), "visibilitychange"),
        (window.clone().unchecked_into::<web_sys::EventTarget>(), "pagehide"),
    ] {
        let owner = Rc::downgrade(game);
        let document = document.clone();
        let listener = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            if event.type_() == "visibilitychange" && !document.hidden() {
                return;
            }
            if let Some(game) = owner.upgrade() {
                game.borrow_mut().request_save();
                write_pending_save(&game);
            }
        });
        target
            .add_event_listener_with_callback(name, listener.as_ref().unchecked_ref())
            .map_err(|_| format!("Could not listen for {name} saves"))?;
        game.borrow_mut().retain_save_listener(listener);
    }
    Ok(())
}

fn request_frame(window: &Window, closure: &Closure<dyn FnMut(f64)>) {
    if window.request_animation_frame(closure.as_ref().unchecked_ref()).is_err() {
        web_sys::console::error_1(&JsValue::from_str(
            "requestAnimationFrame failed; the game loop stopped",
        ));
    }
}
