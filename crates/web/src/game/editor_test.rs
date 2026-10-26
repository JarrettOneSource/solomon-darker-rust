//! Website `editor/EditorTestRuntime.tsx`: one anonymous, disposable admission.
//! The embedding document retains its draft; no save slot or resume token belongs here.

use super::*;
use serde::Deserialize;
use serde_json::json;
use solomon_protocol::hosted::{HostedCommand, HostedHello, HostedServerMessage};
use solomon_sim::settings::game_online_preferences;
use solomon_sim::social::PlayerSocialProfile;
use wasm_bindgen::JsValue;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EditorTestRequest {
    name: String,
    bytes_base64: String,
    account_username: Option<String>,
    cancelled: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorTestPhase {
    Connecting,
    Starting,
    Playing,
    Disposed,
}

pub(crate) fn configured(document: &web_sys::Document) -> Result<bool, String> {
    document
        .query_selector("meta[name='solomon-editor-test']")
        .map(|element| element.is_some())
        .map_err(|_| "Could not read the private test configuration".into())
}

fn request(window: &web_sys::Window) -> Result<EditorTestRequest, String> {
    let value = js_sys::Reflect::get(window, &JsValue::from_str("__solomonEditorTest"))
        .map_err(|_| "Could not read the private test request")?;
    let text = js_sys::JSON::stringify(&value)
        .map_err(|_| "The private test request is invalid")?
        .as_string()
        .ok_or("The private test request is missing")?;
    serde_json::from_str(&text)
        .map_err(|error| format!("The private test request is invalid: {error}"))
}

fn cancelled() -> Result<bool, String> {
    let window = web_sys::window().ok_or("no window")?;
    let value = js_sys::Reflect::get(&window, &JsValue::from_str("__solomonEditorTest"))
        .map_err(|_| "Could not read the private test request")?;
    js_sys::Reflect::get(&value, &JsValue::from_str("cancelled"))
        .map_err(|_| "Could not read private test cancellation")?
        .as_bool()
        .ok_or_else(|| "The private test cancellation state is invalid".into())
}

fn post(message: serde_json::Value) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let parent = window
        .parent()
        .map_err(|_| "The private test has no parent")?
        .ok_or("The private test has no parent")?;
    let origin = window.location().origin().map_err(|_| "The private test has no origin")?;
    let message =
        js_sys::JSON::parse(&message.to_string()).map_err(|_| "Could not encode test status")?;
    parent
        .post_message(&message, &origin)
        .map_err(|_| "Could not report private test status".into())
}

pub(crate) fn report_failure(message: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    if !configured(&window.document().ok_or("no document")?)? {
        return Ok(());
    }
    post(json!({"type":"solomon-editor-test-state","state":"error","message":message}))?;
    post(json!({"type":"solomon-editor-test-disposed"}))
}

impl Game {
    pub(super) async fn load_editor_test(
        &mut self,
        window: &web_sys::Window,
    ) -> Result<(), String> {
        let request = request(window)?;
        self.editor_test = Some(EditorTestPhase::Connecting);
        self.native = Some(NativeGame::idle());
        self.menu.hide()?;
        self.menu.set_settings_account_username(request.account_username.as_deref())?;
        if request.cancelled {
            return self.dispose_editor_test();
        }
        let mount = crate::hosted::WebsiteMount::configured(window)?
            .ok_or("The private test has no Website configuration")?;
        let api = crate::hosted::WebsiteApi::new(
            &window.location().href().map_err(|_| "Game location")?,
            mount,
        )?;
        post(
            json!({"type":"solomon-editor-test-state","state":"loading","message":"Starting a private test"}),
        )?;
        let endpoint = api
            .admit_editor_test(&request.name, &request.bytes_base64)
            .await
            .map_err(|error| error.to_string())?;
        let settings = self.menu.settings();
        let mut online_preferences = game_online_preferences(settings);
        online_preferences.submit_runs = false;
        let hello = HostedHello {
            version: solomon_protocol::PROTOCOL_VERSION,
            credential: endpoint.credential,
            selection: NewGameSelection {
                name: "Test Wizard".into(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
            profile: PlayerSocialProfile {
                account_username: None,
                highest_wave: None,
                total_playtime_ms: Some(0),
            },
            online_preferences,
            resume_token: None,
            save: None,
            save_intent: None,
            begin_college_intro: false,
            decline_tutorial: false,
            cheats_enabled: false,
            enhanced_effects: settings.enhanced_effects,
            allow_mod_mismatch: false,
        };
        // Cancellation after admission must still claim the disposable authority.
        // The regular Welcome path owns the handshake before we close its socket.
        self.begin_hosted_connection(
            &endpoint.url,
            ClientMessage::Hosted { command: HostedCommand::Hello { hello } },
            false,
        )
    }

    pub fn is_editor_test(&self) -> bool {
        self.editor_test.is_some()
    }

    pub fn editor_test_disposed(&self) -> bool {
        self.editor_test == Some(EditorTestPhase::Disposed)
    }

    pub(super) fn poll_editor_test(&mut self) -> Result<(), String> {
        let Some(phase) = self.editor_test else { return Ok(()) };
        if phase == EditorTestPhase::Disposed {
            return Ok(());
        }
        let native = self.native.as_mut().expect("an editor test owns its connection");
        for (message, _) in std::mem::take(&mut native.hosted_messages) {
            if matches!(message, HostedServerMessage::DeploymentRestart { .. }) {
                return Err("The game was updated. Return to editing and start a new test.".into());
            }
        }
        let Some(welcome) = native.hosted_welcome.as_ref() else { return Ok(()) };
        if cancelled()? {
            return self.dispose_editor_test();
        }
        if phase == EditorTestPhase::Connecting {
            if !welcome.boneyards.iter().any(|choice| choice.id == "editor-test") {
                return Err("The private test did not receive your Boneyard.".into());
            }
            self.send_native_message(&ClientMessage::Hosted {
                command: HostedCommand::StartMatch { boneyard_id: "editor-test".into() },
            })?;
            self.editor_test = Some(EditorTestPhase::Starting);
        } else if phase == EditorTestPhase::Starting
            && self.session.boneyard.is_some()
            && !self.world_loading
        {
            self.editor_test = Some(EditorTestPhase::Playing);
            post(json!({"type":"solomon-editor-test-state","state":"playing"}))?;
        }
        Ok(())
    }

    pub(super) fn return_to_editor(&mut self) -> Result<(), String> {
        post(json!({"type":"solomon-editor-test-return"}))?;
        self.dispose_editor_test()
    }

    pub(super) fn dispose_editor_test(&mut self) -> Result<(), String> {
        self.native = Some(NativeGame::idle());
        self.editor_test = Some(EditorTestPhase::Disposed);
        self.has_game = false;
        self.pending_world = None;
        self.world_generation += 1;
        self.world_loading = false;
        self.match_loading = None;
        self.audio.stop_effects()?;
        self.audio.set_music_paused(true)?;
        post(json!({"type":"solomon-editor-test-disposed"}))
    }

    pub fn editor_test_failed(&mut self, message: &str) -> Result<(), String> {
        if self.editor_test.is_none() {
            return Ok(());
        }
        post(json!({"type":"solomon-editor-test-state","state":"error","message":message}))?;
        self.dispose_editor_test()
    }

    /// Disposal must progress while an embedding page has hidden this frame.
    pub fn editor_test_cleanup(&mut self) -> Result<bool, String> {
        if self.editor_test_disposed() {
            return Ok(true);
        }
        if !self.is_editor_test() {
            return Ok(false);
        }
        let now = web_sys::window()
            .and_then(|window| window.performance())
            .ok_or("The private test has no browser clock")?
            .now();
        self.poll_native(now)?;
        self.poll_editor_test()?;
        Ok(self.editor_test_disposed())
    }
}
