//! Source `desktop-runtime.ts`, `game-bootstrap.ts` and `DesktopUpdateNotice.tsx`.
//! The preload owns process/network capabilities. This module only borrows those
//! functions and owns the update notice and subscription lifetimes.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Document, Element, Event, HtmlElement, Window};

use crate::ui::{append, element};

/// Credentials are intentionally absent from Debug/log output.
#[derive(Clone)]
pub struct DesktopEndpoint {
    pub url: String,
    pub credential: String,
}

pub fn configured(window: &Window) -> Result<Option<DesktopEndpoint>, String> {
    let Some(runtime) = runtime(window)? else { return Ok(None) };
    let endpoint = field(&runtime, "gameEndpoint")?;
    if endpoint.is_undefined() || endpoint.is_null() {
        return Ok(None);
    }
    if field(&endpoint, "kind")?.as_string().as_deref() != Some("localhost")
        || field(&endpoint, "sessionKind")?.as_string().as_deref() != Some("standalone")
    {
        return Err("The desktop preload supplied an invalid game endpoint".into());
    }
    let url = field(&endpoint, "url")?.as_string().ok_or("The desktop game endpoint has no URL")?;
    let parsed = web_sys::Url::new(&url).map_err(|_| "The desktop game endpoint URL is invalid")?;
    if parsed.protocol() != "ws:"
        || parsed.hostname() != "127.0.0.1"
        || parsed.pathname() != "/game"
        || parsed.port().parse::<u16>().ok().is_none_or(|port| port == 0)
        || !parsed.username().is_empty()
        || !parsed.password().is_empty()
        || !parsed.search().is_empty()
        || !parsed.hash().is_empty()
    {
        return Err("The desktop game endpoint must name the loopback authority".into());
    }
    let credential = field(&endpoint, "credential")?
        .as_string()
        .filter(|value| !value.is_empty())
        .ok_or("The desktop game endpoint has no credential")?;
    Ok(Some(DesktopEndpoint { url, credential }))
}

struct Notice {
    document: Document,
    root: HtmlElement,
    current_revision: Option<String>,
    dismissed_revision: Option<String>,
    active: bool,
}

impl Notice {
    fn receive(&mut self, value: &JsValue) -> Result<(), String> {
        let update = field(value, "update")?;
        self.current_revision = if update.is_null() || update.is_undefined() {
            None
        } else {
            Some(
                field(&update, "revision")?
                    .as_string()
                    .ok_or("The desktop update revision is invalid")?,
            )
        };
        self.paint()
    }

    fn paint(&self) -> Result<(), String> {
        let visible = self.active
            && self.current_revision.is_some()
            && self.current_revision != self.dismissed_revision;
        if visible && self.root.parent_node().is_none() {
            self.document
                .body()
                .ok_or("The desktop document has no body")?
                .append_child(&self.root)
                .map_err(describe)?;
        } else if !visible {
            // Source conditionally returns null, rather than retaining a hidden
            // interactive notice above the game.
            self.root.remove();
        }
        Ok(())
    }
}

pub struct DesktopBridge {
    notice: Rc<RefCell<Notice>>,
    click: Closure<dyn FnMut(Event)>,
    on_state: Closure<dyn FnMut(JsValue)>,
    remove_state: Option<Function>,
    remove_before_close: Option<Function>,
}

pub fn attach(window: &Window, before_close: &Function) -> Result<Option<DesktopBridge>, String> {
    let Some(desktop) = desktop(window)? else { return Ok(None) };
    let get_state = method(&desktop, "getState")?;
    let subscribe_state = method(&desktop, "onState")?;
    let subscribe_close = method(&desktop, "onBeforeClose")?;
    let download_update = method(&desktop, "downloadUpdate")?;
    let document = window.document().ok_or("The desktop window has no document")?;
    let root = element(&document, "aside", "desktop-update-notice")?;
    root.set_attribute("role", "status").map_err(describe)?;
    let text = element(&document, "span", "")?;
    text.set_text_content(Some("A Solomon Darker update is available."));
    append(&root, &text)?;
    for (label, action) in [("Download update", "download"), ("Later", "later")] {
        let button = element(&document, "button", "")?;
        button.set_attribute("type", "button").map_err(describe)?;
        button.set_attribute("data-desktop-update", action).map_err(describe)?;
        button.set_text_content(Some(label));
        append(&root, &button)?;
    }
    let notice = Rc::new(RefCell::new(Notice {
        document,
        root: root.clone(),
        current_revision: None,
        dismissed_revision: None,
        active: true,
    }));
    let click_notice = Rc::clone(&notice);
    let click_desktop = desktop.clone();
    let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
        let Some(target) = event.target().and_then(|target| target.dyn_into::<Element>().ok())
        else {
            return;
        };
        let Ok(Some(button)) = target.closest("[data-desktop-update]") else { return };
        match button.get_attribute("data-desktop-update").as_deref() {
            Some("later") => {
                let mut notice = click_notice.borrow_mut();
                notice.dismissed_revision = notice.current_revision.clone();
                if let Err(error) = notice.paint() {
                    report(error);
                }
            }
            Some("download") => {
                let result = download_update.call0(&click_desktop);
                spawn_local(async move {
                    if let Err(error) = settle(result).await {
                        report(error);
                    }
                });
            }
            _ => {}
        }
    });
    root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
        .map_err(describe)?;
    let state_notice = Rc::clone(&notice);
    let on_state = Closure::<dyn FnMut(JsValue)>::new(move |value: JsValue| {
        let mut notice = state_notice.borrow_mut();
        if notice.active
            && let Err(error) = notice.receive(&value)
        {
            report(error);
        }
    });
    let mut bridge =
        DesktopBridge { notice, click, on_state, remove_state: None, remove_before_close: None };
    // Keep partial registration under the same Drop path if the preload throws.
    bridge.remove_before_close = Some(unsubscribe(
        subscribe_close.call1(&desktop, before_close.as_ref()).map_err(describe)?,
        "onBeforeClose",
    )?);
    bridge.remove_state = Some(unsubscribe(
        subscribe_state.call1(&desktop, bridge.on_state.as_ref()).map_err(describe)?,
        "onState",
    )?);
    let pending = get_state.call0(&desktop);
    let initial_notice = Rc::downgrade(&bridge.notice);
    spawn_local(async move {
        let result = settle(pending).await;
        let Some(notice) = initial_notice.upgrade() else { return };
        let mut notice = notice.borrow_mut();
        if !notice.active {
            return;
        }
        match result {
            Ok(value) => {
                if let Err(error) = notice.receive(&value) {
                    report(error);
                }
            }
            Err(error) => report(error),
        }
    });
    Ok(Some(bridge))
}

pub async fn open_website(window: &Window) -> Result<(), String> {
    invoke(window, "openWebsite").await
}

pub async fn return_to_launcher(window: &Window) -> Result<(), String> {
    invoke(window, "returnToLauncher").await
}

impl Drop for DesktopBridge {
    fn drop(&mut self) {
        self.notice.borrow_mut().active = false;
        for remove in [&self.remove_state, &self.remove_before_close].into_iter().flatten() {
            if let Err(error) = remove.call0(&JsValue::UNDEFINED) {
                report(describe(error));
            }
        }
        let root = &self.notice.borrow().root;
        if let Err(error) =
            root.remove_event_listener_with_callback("click", self.click.as_ref().unchecked_ref())
        {
            report(describe(error));
        }
        root.remove();
    }
}

async fn invoke(window: &Window, name: &str) -> Result<(), String> {
    let desktop = desktop(window)?.ok_or("This action requires the desktop preload")?;
    settle(method(&desktop, name)?.call0(&desktop)).await?;
    Ok(())
}

async fn settle(value: Result<JsValue, JsValue>) -> Result<JsValue, String> {
    let value = value.map_err(describe)?;
    JsFuture::from(Promise::resolve(&value)).await.map_err(describe)
}

fn runtime(window: &Window) -> Result<Option<JsValue>, String> {
    let value = field(window.as_ref(), "solomonDarkRuntime")?;
    if value.is_undefined() || value.is_null() {
        Ok(None)
    } else if value.is_object() {
        Ok(Some(value))
    } else {
        Err("The desktop preload runtime is invalid".into())
    }
}

fn desktop(window: &Window) -> Result<Option<JsValue>, String> {
    let Some(runtime) = runtime(window)? else { return Ok(None) };
    let value = field(&runtime, "desktop")?;
    if value.is_undefined() || value.is_null() {
        Ok(None)
    } else if value.is_object() {
        Ok(Some(value))
    } else {
        Err("The desktop preload interface is invalid".into())
    }
}

fn field(value: &JsValue, name: &str) -> Result<JsValue, String> {
    Reflect::get(value, &JsValue::from_str(name)).map_err(describe)
}

fn method(value: &JsValue, name: &str) -> Result<Function, String> {
    field(value, name)?
        .dyn_into::<Function>()
        .map_err(|_| format!("The desktop preload does not provide {name}"))
}

fn unsubscribe(value: JsValue, name: &str) -> Result<Function, String> {
    value
        .dyn_into::<Function>()
        .map_err(|_| format!("The desktop {name} subscription has no removal function"))
}

fn describe(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| error.dyn_ref::<js_sys::Error>().map(|error| error.message().into()))
        .unwrap_or_else(|| "The desktop preload operation failed".into())
}

fn report(error: String) {
    web_sys::console::error_1(&JsValue::from_str(&error));
}
