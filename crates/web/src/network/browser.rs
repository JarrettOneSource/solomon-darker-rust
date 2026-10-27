use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket, Window};

use super::connection::{ConnectionCore, Delivery, encode};
use super::{ClientMessage, MAX_PENDING_SEND_BYTES, NetworkPoll};

pub(super) struct NativeConnection {
    state: Rc<RefCell<BrowserConnection>>,
    // Callbacks detach before their owner retires; weak captures do not retain it.
    _callbacks: Vec<Closure<dyn FnMut(Event)>>,
}

struct BrowserConnection {
    window: Window,
    socket: WebSocket,
    core: ConnectionCore,
    timer: Option<i32>,
    timer_callback: Option<Closure<dyn FnMut()>>,
}

impl BrowserConnection {
    fn send(&self, message: &ClientMessage) -> Result<(), String> {
        if self.socket.ready_state() != WebSocket::OPEN {
            return Err("The game host is not connected".into());
        }
        if self.socket.buffered_amount() as usize > MAX_PENDING_SEND_BYTES {
            return Err("The game connection is not accepting input".into());
        }
        self.socket
            .send_with_str(&encode(message)?)
            .map_err(|_| "Could not send the game command".into())
    }

    fn clear_timer(&mut self) {
        if let Some(handle) = self.timer.take() {
            self.window.clear_timeout_with_handle(handle);
        }
    }

    fn arm_timer(&mut self) -> Result<(), String> {
        self.clear_timer();
        let Some(at) = self.core.next_deadline() else { return Ok(()) };
        let delay = (at - crate::platform::monotonic_ms()).ceil().max(0.0) as i32;
        let handle = self
            .window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                self.timer_callback
                    .as_ref()
                    .expect("installed connection timer")
                    .as_ref()
                    .unchecked_ref(),
                delay,
            )
            .map_err(|_| "The game connection timer could not start")?;
        self.timer = Some(handle);
        Ok(())
    }

    fn fail(&mut self, reason: String) {
        self.core.fail(reason);
        self.clear_timer();
        if self.socket.ready_state() < WebSocket::CLOSING
            && let Err(error) =
                self.socket.close_with_code_and_reason(4003, "invalid host delivery")
        {
            crate::platform::warn(&format!(
                "Could not close the invalid game connection: {error:?}"
            ));
        }
    }

    fn receive(&mut self, event: &Event, now: f64) -> Result<(), String> {
        let text = event
            .dyn_ref::<MessageEvent>()
            .and_then(|event| event.data().as_string())
            .ok_or("The game host sent a non-text message")?;
        if let Some(message) = self.core.receive(&text, now)? {
            self.send(&message)?;
        }
        self.arm_timer()
    }
}

impl NativeConnection {
    pub fn connect_to(
        endpoint: &str,
        _origin: &str,
        first_message: ClientMessage,
    ) -> Result<Self, String> {
        let window = web_sys::window().ok_or("no window")?;
        let socket = WebSocket::new(endpoint).map_err(|_| "Could not connect to the game host")?;
        let welcome = !matches!(first_message, ClientMessage::InspectSession { .. });
        let delivery = Arc::new(Mutex::new(Delivery::default()));
        let state = Rc::new(RefCell::new(BrowserConnection {
            window,
            socket: socket.clone(),
            core: ConnectionCore::new(delivery, welcome, crate::platform::monotonic_ms()),
            timer: None,
            timer_callback: None,
        }));
        let ticking = Rc::downgrade(&state);
        state.borrow_mut().timer_callback = Some(Closure::<dyn FnMut()>::new(move || {
            let Some(state) = ticking.upgrade() else { return };
            let mut state = state.borrow_mut();
            state.timer = None;
            let result = state.core.tick(crate::platform::monotonic_ms()).and_then(|message| {
                if let Some(message) = message {
                    state.send(&message)?;
                }
                state.arm_timer()
            });
            if let Err(error) = result {
                state.fail(error);
            }
        }));
        state.borrow_mut().arm_timer()?;
        let mut callbacks: Vec<Closure<dyn FnMut(Event)>> = Vec::with_capacity(4);

        let opened = Rc::downgrade(&state);
        let mut first_message = Some(first_message);
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            if let Some(state) = opened.upgrade() {
                let mut state = state.borrow_mut();
                if let Some(message) = first_message.take() {
                    match state.send(&message) {
                        Ok(()) => state.core.opened(),
                        Err(error) => state.fail(error),
                    }
                }
            }
        });
        socket.set_onopen(Some(callback.as_ref().unchecked_ref()));
        callbacks.push(callback);

        let received = Rc::downgrade(&state);
        let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let now = crate::platform::monotonic_ms();
            if let Some(state) = received.upgrade() {
                let mut state = state.borrow_mut();
                if let Err(error) = state.receive(&event, now) {
                    state.fail(error);
                }
            }
        });
        socket.set_onmessage(Some(callback.as_ref().unchecked_ref()));
        callbacks.push(callback);

        let closed = Rc::downgrade(&state);
        let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            if let Some(state) = closed.upgrade() {
                let mut state = state.borrow_mut();
                state.clear_timer();
                if let Some(event) = event.dyn_ref::<CloseEvent>() {
                    state.core.closed(event.code(), event.reason());
                }
            }
        });
        socket.set_onclose(Some(callback.as_ref().unchecked_ref()));
        callbacks.push(callback);

        let failed = Rc::downgrade(&state);
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            if let Some(state) = failed.upgrade() {
                state.borrow_mut().fail("The game connection was interrupted".into());
            }
        });
        socket.set_onerror(Some(callback.as_ref().unchecked_ref()));
        callbacks.push(callback);
        Ok(Self { state, _callbacks: callbacks })
    }

    pub fn poll(&self) -> NetworkPoll {
        std::mem::take(&mut self.state.borrow().core.delivery.lock().unwrap().poll)
    }

    pub fn is_open(&self) -> bool {
        self.state.borrow().core.delivery.lock().unwrap().open
    }

    pub fn ping_ms(&self) -> Option<f64> {
        self.state.borrow().core.delivery.lock().unwrap().ping_ms
    }

    pub fn send(&self, message: &ClientMessage) -> Result<(), String> {
        self.state.borrow().send(message)
    }

    pub fn accept_welcome(&self) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        if let Some(message) = state.core.accept_welcome() {
            state.send(&message)?;
        }
        state.arm_timer()
    }

    pub fn start_ping(&self, observer: bool) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        let message = state.core.start_ping(crate::platform::monotonic_ms(), observer);
        state.send(&message)?;
        state.arm_timer()
    }
}

impl Drop for NativeConnection {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.clear_timer();
        state.socket.set_onopen(None);
        state.socket.set_onmessage(None);
        state.socket.set_onclose(None);
        state.socket.set_onerror(None);
        if state.socket.ready_state() < WebSocket::CLOSING
            && let Err(error) = state.socket.close()
        {
            crate::platform::warn(&format!("Could not close the game connection: {error:?}"));
        }
    }
}
