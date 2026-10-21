//! Source `deployment-revision.ts` and `GameDeploymentUpdate.tsx`.
//! This page owner survives the announced transport close and reloads only a new
//! deployed revision, or the exact announced target when the server requests one.

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    AbortController, AbortSignal, Document, HtmlElement, RequestCache, RequestInit, Response,
    Window,
};

use crate::hosted::WebsiteMount;
use crate::ui::{append, element};

pub struct DeploymentUpdate {
    root: HtmlElement,
    message: HtmlElement,
    mount: WebsiteMount,
    revision: Option<String>,
    polling: Option<AbortController>,
    restarting: bool,
}

impl DeploymentUpdate {
    pub fn new(
        document: &Document,
        mount: WebsiteMount,
        watch_server: bool,
    ) -> Result<Self, String> {
        let mount = match document
            .query_selector("meta[name='solomon-game-mount']")
            .map_err(|_| "Could not read the game artifact mount")?
        {
            Some(meta) => WebsiteMount::new(&meta.get_attribute("content").unwrap_or_default())?,
            None => mount,
        };
        let revision = document
            .query_selector("meta[name='solomon-build-revision']")
            .map_err(|_| "Could not read the game build revision")?
            .and_then(|meta| meta.get_attribute("content"))
            .and_then(|revision| watch_server.then(|| normalized_revision(&revision)).flatten());
        let root = element(document, "div", "game-deployment-update")?;
        root.set_hidden(true);
        root.set_attribute("role", "status").map_err(|_| "Update status role")?;
        root.set_attribute("aria-live", "assertive").map_err(|_| "Update announcement")?;
        let css = element(document, "style", "game-deployment-update-style")?;
        css.set_text_content(Some(include_str!("../../../web/game-deployment-update.css")));
        append(&root, &css)?;
        let panel = element(document, "section", "game-deployment-update-panel")?;
        let kicker = element(document, "p", "game-deployment-update-kicker")?;
        kicker.set_text_content(Some("Server update"));
        append(&panel, &kicker)?;
        let title = element(document, "h1", "")?;
        title.set_text_content(Some("Game updating"));
        append(&panel, &title)?;
        let message = element(document, "p", "")?;
        append(&panel, &message)?;
        append(&root, &panel)?;
        append(&document.body().ok_or("The game update needs a document body")?, &root)?;
        let mut update = Self { root, message, mount, revision, polling: None, restarting: false };
        update.poll_revision(None)?;
        Ok(update)
    }

    pub fn restarting(&self) -> bool {
        self.restarting
    }

    pub fn begin(&mut self, target: &str) -> Result<(), String> {
        if self.restarting {
            return Ok(());
        }
        self.restarting = true;
        self.root.set_hidden(false);
        self.message.set_text_content(Some(
            "Saving your game before the update. Solomon Dark will restart automatically.",
        ));
        self.poll_revision(Some(target.to_owned()))
    }

    pub fn saved(&mut self) -> Result<(), String> {
        self.message.set_text_content(Some(
            "Your game is saved. Solomon Dark will restart automatically when the update is ready.",
        ));
        // Source restarts the polling effect when the saved state object changes.
        let target = self.root.get_attribute("data-target-revision");
        self.poll_revision(target)
    }

    fn poll_revision(&mut self, target: Option<String>) -> Result<(), String> {
        if let Some(previous) = self.polling.take() {
            previous.abort();
        }
        if let Some(target) = &target {
            self.root
                .set_attribute("data-target-revision", target)
                .map_err(|_| "Update revision")?;
        }
        let Some(current) = self.revision.clone() else { return Ok(()) };
        let controller =
            AbortController::new().map_err(|_| "Could not watch the deployed revision")?;
        let signal = controller.signal();
        let url = self.mount.path(&format!("/deployment.json?current={current}"));
        let interval = if self.restarting { 500 } else { 15_000 };
        spawn_local(async move {
            let result =
                wait_for_revision(&url, &current, target.as_deref(), interval, &signal).await;
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
        self.root.remove();
    }
}

async fn wait_for_revision(
    url: &str,
    current: &str,
    target: Option<&str>,
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
                // Invalid/missing deployment documents are ignored by Source, including
                // an HTML cache fallback or a JSON body interrupted by a cutover.
                if response.ok()
                    && let Ok(body) = JsFuture::from(response.text()?).await
                    && let Some(body) = body.as_string()
                    && let Some(live) = deployment_revision(response.status(), &body)
                    && should_reload(current, &live, target)
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

fn deployment_revision(status: u16, body: &str) -> Option<String> {
    if !(200..300).contains(&status) {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    normalized_revision(value.as_object()?.get("revision")?.as_str()?)
}

fn should_reload(current: &str, live: &str, target: Option<&str>) -> bool {
    live != current && target.is_none_or(|target| live == target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn revision_response_and_exact_target_match_the_source() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../sim/tests/fixtures/website/deployment-revision-goldens.json"
        ))
        .unwrap();
        for row in fixture["responses"].as_array().unwrap() {
            let actual = deployment_revision(
                row["status"].as_u64().unwrap() as u16,
                row["body"].as_str().unwrap(),
            );
            assert_eq!(serde_json::to_value(actual).unwrap(), row["expected"], "{row}");
        }
        for row in fixture["reloads"].as_array().unwrap() {
            assert_eq!(
                should_reload(
                    row["current"].as_str().unwrap(),
                    row["live"].as_str().unwrap(),
                    row["target"].as_str()
                ),
                row["expected"].as_bool().unwrap(),
                "{row}"
            );
        }
    }
}
