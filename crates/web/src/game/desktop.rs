//! The desktop close request uses the same source checkpoint and durable revision owner
//! as ordinary Save and Leave. A transport-only timer also works while rAF is suspended.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use js_sys::Promise;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, future_to_promise};

use super::Game;
use crate::menu::MenuAction;

pub(super) enum DesktopClose {
    Saving,
    Complete(Result<(), String>),
}

impl Game {
    pub(crate) fn attach_desktop(
        game: &Rc<RefCell<Self>>,
        window: &web_sys::Window,
    ) -> Result<(), String> {
        if crate::desktop::configured(window)?.is_none() {
            return Ok(());
        }
        let owner = Rc::downgrade(game);
        let pending = Rc::new(RefCell::new(None::<Promise>));
        let callback = Closure::<dyn FnMut() -> Promise>::new(move || {
            if let Some(promise) = pending.borrow().as_ref() {
                return promise.clone();
            }
            let owner = owner.clone();
            let completed = pending.clone();
            let promise = future_to_promise(async move {
                let result = save_before_close(owner).await;
                completed.borrow_mut().take();
                result
                    .map(|()| JsValue::UNDEFINED)
                    .map_err(|message| JsValue::from(js_sys::Error::new(&message)))
            });
            *pending.borrow_mut() = Some(promise.clone());
            promise
        });
        let bridge = crate::desktop::attach(window, callback.as_ref().unchecked_ref())?;
        let mut game = game.borrow_mut();
        game.desktop_bridge = bridge;
        game.desktop_close_callback = Some(callback);
        Ok(())
    }

    fn begin_desktop_close(&mut self) -> Result<bool, String> {
        let Some(native) = &self.native else { return Ok(false) };
        if !native.is_playing() && !native.is_leaving() {
            return Ok(false);
        }
        self.desktop_close = Some(DesktopClose::Saving);
        if !native.is_leaving() {
            self.native_menu_action(&MenuAction::Leave)?;
        }
        Ok(true)
    }

    pub(super) fn finish_desktop_close(&mut self, result: Result<(), String>) {
        if matches!(self.desktop_close, Some(DesktopClose::Saving)) {
            self.desktop_close = Some(DesktopClose::Complete(result));
        }
    }

    fn poll_desktop_close(&mut self, now: f64) -> Result<bool, String> {
        if matches!(self.desktop_close, Some(DesktopClose::Saving)) {
            self.poll_native(now)?;
            self.poll_hosted(now)?;
        }
        if matches!(self.desktop_close, Some(DesktopClose::Complete(_))) {
            let Some(DesktopClose::Complete(result)) = self.desktop_close.take() else {
                unreachable!("the completed desktop close owns its result")
            };
            return result.map(|()| true);
        }
        Ok(false)
    }
}

async fn save_before_close(owner: Weak<RefCell<Game>>) -> Result<(), String> {
    let game = owner.upgrade().ok_or("The game is no longer available to save.")?;
    if !game.borrow_mut().begin_desktop_close()? {
        return Ok(());
    }
    let window = web_sys::window().ok_or("The game window is unavailable.")?;
    let clock = window.performance().ok_or("The game clock is unavailable.")?;
    loop {
        if game.borrow_mut().poll_desktop_close(clock.now())? {
            return Ok(());
        }
        crate::run_hosted_work(&game);
        let pause = Promise::new(&mut |resolve, reject| {
            if let Err(error) =
                window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 20)
            {
                let _ = reject.call1(&JsValue::UNDEFINED, &error);
            }
        });
        JsFuture::from(pause).await.map_err(|_| "The game could not wait for its final save.")?;
    }
}
