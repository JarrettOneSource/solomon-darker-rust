//! Source `deployment-revision.ts` and `GameDeploymentUpdate.tsx`.
//! The page owner survives transport retirement. A publication identity gates the
//! reload independently of the game revision bound into signed recovery claims.

use std::{cell::RefCell, rc::Rc};

mod state;
use state::{DeploymentState, Publication};

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    AbortController, AbortSignal, Document, RequestCache, RequestInit, Response, Window,
};

use crate::gpu_ui::UiFrame;
use crate::hosted::WebsiteMount;
use egui::{Color32, FontFamily, FontId, Id, Order, RichText, Stroke, vec2};

pub struct DeploymentUpdate {
    message: Option<&'static str>,
    target_revision: Option<String>,
    mount: WebsiteMount,
    state: Option<Rc<RefCell<DeploymentState>>>,
    polling: Option<AbortController>,
    restarting: bool,
}

impl DeploymentUpdate {
    pub fn new(
        document: &Document,
        mount: WebsiteMount,
        watch_server: bool,
    ) -> Result<Self, String> {
        let revision = document
            .query_selector("meta[name='solomon-build-revision']")
            .map_err(|_| "Could not read the game build revision")?
            .and_then(|meta| meta.get_attribute("content"))
            .and_then(|revision| watch_server.then(|| normalized_revision(&revision)).flatten());
        let mut update = Self {
            message: None,
            target_revision: None,
            mount,
            state: revision.map(|revision| Rc::new(RefCell::new(DeploymentState::new(revision)))),
            polling: None,
            restarting: false,
        };
        update.poll_publication()?;
        Ok(update)
    }

    pub fn restarting(&self) -> bool {
        self.restarting
    }

    pub fn begin(
        &mut self,
        target: &str,
        deployment_id: &str,
        deployment_sequence: u64,
    ) -> Result<(), String> {
        if self.restarting {
            return Ok(());
        }
        let target = normalized_revision(target).ok_or("The deployment target is invalid")?;
        normalized_deployment_id(deployment_id)
            .ok_or("The deployment publication identity is invalid")?;
        if deployment_sequence == 0 {
            return Err("The deployment publication sequence is invalid".into());
        }
        self.restarting = true;
        self.target_revision = Some(target.clone());
        if let Some(state) = &self.state {
            state.borrow_mut().begin(target, deployment_sequence);
        }
        self.message =
            Some("Saving your game before the update. Solomon Dark will restart automatically.");
        self.poll_publication()
    }

    pub fn saved(&mut self) {
        self.message = Some(
            "Your game is saved. Solomon Dark will restart automatically when the update is ready.",
        );
        if let Some(state) = &self.state {
            state.borrow_mut().saved();
        }
    }

    pub fn transport_retired(&mut self) {
        if let Some(state) = &self.state {
            state.borrow_mut().transport_retired();
        }
    }

    pub fn set_session_active(&mut self, active: bool) {
        if let Some(state) = &self.state {
            state.borrow_mut().set_session_active(active);
        }
    }

    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        if !self.restarting {
            return Ok(());
        }
        let Some(message) = self.message else { return Ok(()) };
        frame.input_scope("deployment/update");
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        let area = egui::Area::new(Id::unique("deployment/update"))
            .order(Order::Foreground)
            .fixed_pos(egui::Pos2::ZERO)
            .show(&ctx, |ui| {
                ui.memory_mut(|memory| memory.set_modal_layer(ui.layer_id()));
                ui.set_min_size(screen.size());
                ui.interact(screen, Id::unique("deployment/block"), egui::Sense::CLICK);
                ui.painter().rect_filled(
                    screen,
                    0.0,
                    Color32::from_rgba_unmultiplied(4, 4, 6, 224),
                );
                let width = (screen.width() - 48.0).min(520.0).max(120.0);
                let height = 240.0_f32.min(screen.height() - 48.0);
                let bounds = egui::Rect::from_center_size(screen.center(), vec2(width, height));
                egui::ScrollArea::vertical()
                    .id_salt("deployment/status-scroll")
                    .max_height(screen.height() - 48.0)
                    .show(ui, |ui| {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
                            egui::Frame::new()
                                .fill(Color32::from_rgb(12, 11, 10))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(128, 109, 63)))
                                .inner_margin(egui::Margin::symmetric(34, 30))
                                .show(ui, |ui| {
                                    ui.set_width((width - 70.0).max(50.0));
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            RichText::new("SERVER UPDATE")
                                                .font(FontId::new(
                                                    12.0,
                                                    FontFamily::Name("Cinzel".into()),
                                                ))
                                                .color(Color32::from_rgb(157, 137, 83)),
                                        );
                                        ui.add_space(8.0);
                                        ui.label(
                                            RichText::new("Game updating")
                                                .font(FontId::new(
                                                    32.0,
                                                    FontFamily::Name("Georgia".into()),
                                                ))
                                                .color(Color32::from_rgb(240, 217, 142)),
                                        );
                                        ui.add_space(14.0);
                                        ui.label(
                                            RichText::new(message)
                                                .font(FontId::new(
                                                    16.0,
                                                    FontFamily::Name("Alegreya".into()),
                                                ))
                                                .color(Color32::from_rgb(201, 190, 162)),
                                        );
                                    });
                                });
                        });
                    });
                bounds
            });
        frame.record_rect("deployment/update", message, "status", area.inner, true);
        frame.record_value(
            "deployment/update",
            self.target_revision.as_deref().unwrap_or(""),
            None,
        );
        Ok(())
    }

    fn poll_publication(&mut self) -> Result<(), String> {
        if let Some(previous) = self.polling.take() {
            previous.abort();
        }
        let Some(state) = self.state.clone() else { return Ok(()) };
        let controller =
            AbortController::new().map_err(|_| "Could not watch the deployed revision")?;
        let signal = controller.signal();
        let url = self.mount.path("/api/game/deployment");
        let interval = if self.restarting { 500 } else { 15_000 };
        spawn_local(async move {
            let result = wait_for_publication(&url, state, interval, &signal).await;
            if signal.aborted() {
                return;
            }
            match result {
                Ok(()) => {
                    if let Some(window) = web_sys::window()
                        && let Err(error) = window.location().reload()
                    {
                        web_sys::console::warn_1(&error);
                    }
                }
                Err(error) => web_sys::console::warn_2(
                    &JsValue::from_str("The game could not verify the deployed revision."),
                    &error,
                ),
            }
        });
        self.polling = Some(controller);
        Ok(())
    }
}

impl Drop for DeploymentUpdate {
    fn drop(&mut self) {
        if let Some(polling) = &self.polling {
            polling.abort();
        }
    }
}

async fn wait_for_publication(
    url: &str,
    state: Rc<RefCell<DeploymentState>>,
    interval: i32,
    signal: &AbortSignal,
) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("The game has no window"))?;
    while !signal.aborted() {
        let init = RequestInit::new();
        init.set_cache(RequestCache::NoStore);
        init.set_signal(Some(signal));
        let request = web_sys::Request::new_with_str_and_init(url, &init)?;
        request.headers().set("accept", "application/json")?;
        match JsFuture::from(window.fetch_with_request(&request)).await {
            Ok(response) => {
                let response: Response = response.dyn_into()?;
                // Incomplete publication documents never authorize a reload, including
                // an HTML cache fallback or a JSON body interrupted by a cutover.
                if response.ok()
                    && let Ok(body) = JsFuture::from(response.text()?).await
                    && let Some(body) = body.as_string()
                    && let Some(publication) = deployment_publication(response.status(), &body)
                    && state.borrow_mut().observe(&publication)
                {
                    return Ok(());
                }
            }
            Err(error) => {
                if signal.aborted() {
                    return Ok(());
                }
                if !error.is_instance_of::<js_sys::TypeError>() {
                    return Err(error);
                }
            }
        }
        abortable_delay(&window, interval, signal).await?;
    }
    Ok(())
}

async fn abortable_delay(
    window: &Window,
    duration: i32,
    signal: &AbortSignal,
) -> Result<(), JsValue> {
    if signal.aborted() {
        return Ok(());
    }
    let mut resolve = None;
    let promise = js_sys::Promise::new(&mut |done, _| resolve = Some(done));
    let resolve = resolve.expect("Promise installs its resolver synchronously");
    let done = Closure::<dyn FnMut()>::new(move || {
        let _ = resolve.call0(&JsValue::UNDEFINED);
    });
    let timer = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        done.as_ref().unchecked_ref(),
        duration,
    )?;
    if let Err(error) =
        signal.add_event_listener_with_callback("abort", done.as_ref().unchecked_ref())
    {
        window.clear_timeout_with_handle(timer);
        return Err(error);
    }
    let result = JsFuture::from(promise).await;
    window.clear_timeout_with_handle(timer);
    signal.remove_event_listener_with_callback("abort", done.as_ref().unchecked_ref())?;
    result.map(|_| ())
}

fn normalized_revision(revision: &str) -> Option<String> {
    let revision = revision.trim().to_lowercase();
    (revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(revision)
}

fn normalized_deployment_id(id: &str) -> Option<String> {
    let id = id.trim().to_lowercase();
    (id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(id)
}

fn deployment_publication(status: u16, body: &str) -> Option<Publication> {
    if !(200..300).contains(&status) {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let value = value.as_object()?;
    normalized_revision(value.get("revision")?.as_str()?)?;
    normalized_deployment_id(value.get("deploymentId")?.as_str()?)?;
    Some(Publication {
        game_revision: normalized_revision(value.get("gameRevision")?.as_str()?)?,
        sequence: value.get("deploymentSequence")?.as_u64()?,
        ready: value.get("ready")?.as_bool()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_complete_publication_documents_enter_the_lifecycle_barrier() {
        let game = "a".repeat(40);
        let id = "b".repeat(32);
        let publication = serde_json::json!({
            "revision":"c".repeat(40), "gameRevision":game, "deploymentId":id, "deploymentSequence":2, "ready":true,
        });
        assert!(deployment_publication(200, &publication.to_string()).is_some());
        assert!(deployment_publication(503, &publication.to_string()).is_none());
        for field in ["revision", "gameRevision", "deploymentId", "deploymentSequence", "ready"] {
            let mut incomplete = publication.clone();
            incomplete.as_object_mut().unwrap().remove(field);
            assert!(deployment_publication(200, &incomplete.to_string()).is_none(), "{field}");
        }
        for body in
            ["<html>updating</html>", r#"{"revision":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#]
        {
            assert!(deployment_publication(200, body).is_none());
        }
    }
}
