//! Window capture shortcuts run in registration order before the GPU editor.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, serde::Serialize)]
pub struct KeyStroke {
    pub code: String,
    pub key: String,
    pub pressed: bool,
    pub repeat: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
}

pub struct KeyboardEvent<'a> {
    pub stroke: &'a KeyStroke,
    pub editing: bool,
    pub default_prevented: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyboardEffect {
    Pass,
    PreventDefault,
    Stop,
}

type Handler = RefCell<Box<dyn FnMut(&KeyboardEvent<'_>) -> KeyboardEffect>>;

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use wasm_bindgen::{JsCast, closure::Closure};

    struct Listener {
        name: &'static str,
        callback: Closure<dyn FnMut(web_sys::KeyboardEvent)>,
    }
    pub struct KeyboardListener {
        window: web_sys::Window,
        listeners: Vec<Listener>,
    }

    impl KeyboardListener {
        pub fn attach(
            surface: &crate::platform::Surface,
            handler: impl FnMut(&KeyboardEvent<'_>) -> KeyboardEffect + 'static,
        ) -> Result<Self, String> {
            let window = surface
                .canvas()
                .owner_document()
                .and_then(|document| document.default_view())
                .ok_or("Keyboard window is unavailable")?;
            let handler: Rc<Handler> = Rc::new(RefCell::new(Box::new(handler)));
            let mut owner = Self { window, listeners: Vec::new() };
            for (name, pressed) in [("keydown", true), ("keyup", false)] {
                let handler = handler.clone();
                let callback = Closure::new(move |event: web_sys::KeyboardEvent| {
                    let stroke = KeyStroke {
                        code: event.code(),
                        key: event.key(),
                        pressed,
                        repeat: event.repeat(),
                        alt: event.alt_key(),
                        ctrl: event.ctrl_key(),
                        shift: event.shift_key(),
                        meta: event.meta_key(),
                    };
                    let editing = event
                        .target()
                        .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
                        .is_some_and(|target| {
                            target.is_content_editable()
                                || ["INPUT", "TEXTAREA", "SELECT"]
                                    .contains(&target.tag_name().as_str())
                        });
                    let effect = handler.borrow_mut()(&KeyboardEvent {
                        stroke: &stroke,
                        editing,
                        default_prevented: event.default_prevented(),
                    });
                    if effect != KeyboardEffect::Pass {
                        event.prevent_default();
                    }
                    if effect == KeyboardEffect::Stop {
                        event.stop_immediate_propagation();
                    }
                });
                owner
                    .window
                    .add_event_listener_with_callback_and_bool(
                        name,
                        callback.as_ref().unchecked_ref(),
                        true,
                    )
                    .map_err(|_| format!("Could not attach {name} keyboard input"))?;
                owner.listeners.push(Listener { name, callback });
            }
            Ok(owner)
        }
    }

    impl Drop for KeyboardListener {
        fn drop(&mut self) {
            for Listener { name, callback } in &self.listeners {
                if let Err(error) = self.window.remove_event_listener_with_callback_and_bool(
                    name,
                    callback.as_ref().unchecked_ref(),
                    true,
                ) {
                    crate::platform::error(&format!("Keyboard listener cleanup: {error:?}"));
                }
            }
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub use browser::KeyboardListener;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::rc::Weak;
    use winit::window::WindowId;

    struct Subscription {
        window: WindowId,
        handler: Handler,
    }
    thread_local! { static SUBSCRIPTIONS: RefCell<Vec<Weak<Subscription>>> = const { RefCell::new(Vec::new()) }; }

    pub struct KeyboardListener {
        _subscription: Rc<Subscription>,
    }
    impl KeyboardListener {
        pub fn attach(
            surface: &crate::platform::Surface,
            handler: impl FnMut(&KeyboardEvent<'_>) -> KeyboardEffect + 'static,
        ) -> Result<Self, String> {
            let subscription = Rc::new(Subscription {
                window: surface.window().id(),
                handler: RefCell::new(Box::new(handler)),
            });
            SUBSCRIPTIONS.with_borrow_mut(|listeners| {
                listeners.retain(|listener| listener.strong_count() > 0);
                listeners.push(Rc::downgrade(&subscription));
            });
            Ok(Self { _subscription: subscription })
        }
    }

    pub(crate) fn dispatch_keyboard(window: WindowId, stroke: &KeyStroke, editing: bool) -> bool {
        let subscriptions = SUBSCRIPTIONS.with_borrow(|listeners| {
            listeners.iter().filter_map(Weak::upgrade).collect::<Vec<_>>()
        });
        let mut event = KeyboardEvent { stroke, editing, default_prevented: false };
        for subscription in
            subscriptions.iter().filter(|subscription| subscription.window == window)
        {
            let effect = subscription.handler.borrow_mut()(&event);
            event.default_prevented |= effect != KeyboardEffect::Pass;
            if effect == KeyboardEffect::Stop {
                break;
            }
        }
        event.default_prevented
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use native::KeyboardListener;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::dispatch_keyboard;
