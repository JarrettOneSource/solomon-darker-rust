//! Browser document/bootstrap ownership for the shared Rust client.
use crate::{game, load_pending_world, run_hosted_work, runtime_ui, write_pending_save};
use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::Window;

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

/// Immutable observations of real GPU controls and submitted frames. Browser journeys
/// locate these bounds, then exercise normal pointer, keyboard and controller input.
#[wasm_bindgen]
pub fn gpu_ui_state() -> String {
    if let Some(loading) = runtime_ui::loading_snapshot() {
        return loading;
    }
    GAME.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or_else(|| "null".to_owned(), |game| game.borrow().gpu_ui_snapshot().to_string())
    })
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    crate::platform::spawn_local(async {
        if let Err(message) = run().await {
            web_sys::console::error_1(&JsValue::from_str(&message));
            if let Err(error) = game::editor_test::report_failure(&message) {
                web_sys::console::error_1(&JsValue::from_str(&error));
            }
            runtime_ui::startup_failed(&message);
        }
    });
}

async fn run() -> Result<(), String> {
    let game = Game::load().await?;
    runtime_ui::completed(runtime_ui::RUNTIME)?;
    let window = web_sys::window().ok_or("no window")?;
    GAME.with(|slot| *slot.borrow_mut() = Some(game.clone()));
    Game::attach_browser_input(&game, &window)?;
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
        let Some(game) = slot.as_ref() else {
            return false;
        };
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
pub(crate) type FrameCallback = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

fn schedule_frames(window: Window, game: Rc<RefCell<Game>>) {
    let callback: FrameCallback = Rc::new(RefCell::new(None));
    let next = callback.clone();
    let requester = window.clone();
    *callback.borrow_mut() = Some(Closure::new(move |time: f64| {
        let result = game.borrow_mut().frame(time);
        if let Err(message) = result {
            web_sys::console::error_1(&JsValue::from_str(&message));
            if let Err(error) = game.borrow_mut().report_failure(&message) {
                web_sys::console::error_1(&JsValue::from_str(&error));
            }
            if let Err(error) = game.borrow_mut().present_failure(time) {
                web_sys::console::error_1(&JsValue::from_str(&error));
            }
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

pub(crate) fn request_frame(window: &Window, closure: &Closure<dyn FnMut(f64)>) {
    if window.request_animation_frame(closure.as_ref().unchecked_ref()).is_err() {
        web_sys::console::error_1(&JsValue::from_str(
            "requestAnimationFrame failed; the game loop stopped",
        ));
    }
}
