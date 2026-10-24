//! Source `desktop-runtime.ts`, `game-bootstrap.ts` and `DesktopUpdateNotice.tsx`.
//! The preload owns process/network capabilities. This module only borrows those
//! functions and owns the update notice and subscription lifetimes.

pub(crate) mod launcher;

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::Window;

use crate::gpu_ui::UiFrame;
use egui::{Color32, FontFamily, FontId, RichText, Stroke, vec2};

/// Credentials are intentionally absent from Debug/log output.
#[derive(Clone)]
pub struct DesktopEndpoint {
    pub url: String,
    pub credential: String,
}

pub fn configured(window: &Window) -> Result<Option<DesktopEndpoint>, String> {
    let Some(runtime) = runtime(window)? else {
        return Ok(None);
    };
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
        Ok(())
    }

    fn visible(&self) -> bool {
        self.active
            && self.current_revision.is_some()
            && self.current_revision != self.dismissed_revision
    }
}

pub struct DesktopBridge {
    notice: Rc<RefCell<Notice>>,
    desktop: JsValue,
    download_update: Function,
    on_state: Closure<dyn FnMut(JsValue)>,
    remove_state: Option<Function>,
    remove_before_close: Option<Function>,
}

pub fn attach(window: &Window, before_close: &Function) -> Result<Option<DesktopBridge>, String> {
    let Some(desktop) = desktop(window)? else {
        return Ok(None);
    };
    let get_state = method(&desktop, "getState")?;
    let subscribe_state = method(&desktop, "onState")?;
    let subscribe_close = method(&desktop, "onBeforeClose")?;
    let download_update = method(&desktop, "downloadUpdate")?;
    let notice = Rc::new(RefCell::new(Notice {
        current_revision: None,
        dismissed_revision: None,
        active: true,
    }));
    let state_notice = Rc::clone(&notice);
    let on_state = Closure::<dyn FnMut(JsValue)>::new(move |value: JsValue| {
        let mut notice = state_notice.borrow_mut();
        if notice.active
            && let Err(error) = notice.receive(&value)
        {
            report(error);
        }
    });
    let mut bridge = DesktopBridge {
        notice,
        desktop: desktop.clone(),
        download_update,
        on_state,
        remove_state: None,
        remove_before_close: None,
    };
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
        let Some(notice) = initial_notice.upgrade() else {
            return;
        };
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

impl DesktopBridge {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("desktop/update"))
    }

    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        if !self.notice.borrow().visible() {
            return Ok(());
        }
        let ctx = frame.ctx.clone();
        let mut action = None;
        let area = egui::Area::new(self.layer_id().id)
            .fade_in(false)
            .order(self.layer_id().order)
            .anchor(egui::Align2::RIGHT_TOP, vec2(-12.0, 12.0))
            .show(&ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(30, 24, 36))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(181, 152, 86)))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_max_width((ctx.content_rect().width() - 58.0).max(100.0));
                        ui.spacing_mut().item_spacing.x = 12.0;
                        ui.horizontal_wrapped(|ui| {
                            let copy = |text: &str| {
                                RichText::new(text)
                                    .font(FontId::new(14.0, FontFamily::Name("Georgia".into())))
                                    .color(Color32::from_rgb(242, 230, 199))
                            };
                            ui.label(copy("A Solomon Darker update is available."));
                            for (id, label) in [("download", "Download update"), ("later", "Later")]
                            {
                                let response = ui.add(
                                    egui::Button::new(copy(label))
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(139, 114, 68)))
                                        .corner_radius(3.0)
                                        .min_size(vec2(0.0, 30.0)),
                                );
                                frame.record(&format!("desktop/{id}"), label, "button", &response);
                                if response.clicked() {
                                    action = Some(id);
                                }
                            }
                        });
                    })
                    .response
                    .rect
            });
        frame.record_rect(
            "desktop/update",
            "A Solomon Darker update is available.",
            "status",
            area.inner,
            true,
        );
        match action {
            Some("later") => {
                let mut notice = self.notice.borrow_mut();
                notice.dismissed_revision = notice.current_revision.clone();
            }
            Some("download") => {
                let result = self.download_update.call0(&self.desktop);
                spawn_local(async move {
                    if let Err(error) = settle(result).await {
                        report(error);
                    }
                });
            }
            _ => {}
        }
        Ok(())
    }
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
    let Some(runtime) = runtime(window)? else {
        return Ok(None);
    };
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

#[cfg(test)]
mod tests {
    use super::Notice;

    #[test]
    fn later_dismisses_only_the_displayed_update_and_unmounting_retires_the_notice() {
        let mut notice = Notice { current_revision: None, dismissed_revision: None, active: true };
        assert!(!notice.visible());
        notice.current_revision = Some("first".into());
        assert!(notice.visible());
        notice.dismissed_revision = notice.current_revision.clone();
        assert!(!notice.visible());
        notice.current_revision = Some("second".into());
        assert!(notice.visible());
        notice.active = false;
        assert!(!notice.visible());
    }
}
