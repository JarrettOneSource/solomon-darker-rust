//! Desktop launcher drafts and admission; the native application owns each session.

use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
pub struct Build {
    pub revision: String,
}

#[derive(Clone, Deserialize)]
pub struct Update {
    pub revision: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopState {
    pub build: Build,
    pub addresses: Vec<String>,
    pub update: Option<Update>,
    pub update_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum SessionOptions {
    Solo,
    Host { host: String, port: u16 },
    Join { invite: String },
}

#[derive(Clone)]
pub enum Action {
    Start(SessionOptions),
    Check,
    Download,
    Website,
    Launcher,
    ReloadGame,
    OpenUrl(String),
    CopyInvite,
    KeepPlaying,
    CloseWithoutSaving,
}

pub(super) struct Model {
    pub previous_app_data: bool,
    pub state: DesktopState,
    pub host: String,
    pub port: String,
    pub invite: String,
    pub status: String,
    pub launching: bool,
    pub checking: bool,
    pub help_open: bool,
    pub pending: Option<Action>,
}

impl Model {
    pub fn new(state: DesktopState) -> Self {
        Self {
            previous_app_data: false,
            host: state.addresses.first().cloned().unwrap_or_default(),
            state,
            port: "27843".into(),
            invite: String::new(),
            status: String::new(),
            launching: false,
            checking: false,
            help_open: false,
            pending: None,
        }
    }

    pub fn host_options(&self) -> Result<SessionOptions, String> {
        let host = self.host.trim();
        if host.is_empty() {
            return Err("Enter your reachable IP or hostname.".into());
        }
        let port = self
            .port
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
            .ok_or("Enter a TCP port from 1 to 65535.")?;
        Ok(SessionOptions::Host { host: host.into(), port })
    }

    pub fn join_options(&self) -> Result<SessionOptions, String> {
        let invite = self.invite.trim();
        if invite.is_empty() {
            return Err("Paste your friend's multiplayer invite.".into());
        }
        Ok(SessionOptions::Join { invite: invite.into() })
    }

    pub fn launch(&mut self, options: Result<SessionOptions, String>) {
        if self.launching || self.pending.is_some() {
            return;
        }
        match options {
            Ok(options) => {
                self.status = match options {
                    SessionOptions::Join { .. } => "Connecting to your friend…",
                    _ => "Starting your game…",
                }
                .into();
                self.launching = true;
                self.pending = Some(Action::Start(options));
            }
            Err(error) => self.status = error,
        }
    }

    pub fn receive(&mut self, state: DesktopState) {
        // Periodic update events must never overwrite an in-progress host/invite draft.
        self.state = state;
    }

    pub fn failed(&mut self, error: String) {
        self.launching = false;
        self.checking = false;
        self.status = error;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        Model::new(DesktopState {
            build: Build { revision: "a".repeat(40) },
            addresses: vec!["192.168.1.20".into()],
            update: None,
            update_error: None,
        })
    }

    #[test]
    fn required_host_and_port_are_validated_before_native_admission() {
        let mut model = model();
        for port in ["", "0", "65536", "-1", "3.5", "abc"] {
            model.port = port.into();
            assert!(model.host_options().is_err(), "{port}");
        }
        model.port = "65535".into();
        model.host = "  example.test  ".into();
        assert_eq!(
            model.host_options().unwrap(),
            SessionOptions::Host { host: "example.test".into(), port: 65535 }
        );
        model.host = "  ".into();
        assert!(model.host_options().is_err());
    }

    #[test]
    fn failed_admission_restores_controls_without_discarding_the_invite() {
        let mut model = model();
        model.launch(model.join_options());
        assert!(!model.launching && model.pending.is_none());
        model.invite = " solomondarker://retained-invite ".into();
        model.launch(model.join_options());
        assert!(model.launching);
        assert!(
            matches!(model.pending.take(), Some(Action::Start(SessionOptions::Join { invite })) if invite == "solomondarker://retained-invite")
        );
        model.launch(Ok(SessionOptions::Solo));
        assert!(model.pending.is_none(), "repeated launch cannot create a second host");
        model.failed("The invite belongs to another build.".into());
        assert!(!model.launching);
        assert_eq!(model.invite, " solomondarker://retained-invite ");
        model.launch(Ok(SessionOptions::Solo));
        assert!(matches!(model.pending, Some(Action::Start(SessionOptions::Solo))));
    }

    #[test]
    fn periodic_state_updates_preserve_drafts_and_native_session_options() {
        let mut model = model();
        model.host = "my-host".into();
        model.invite = "draft".into();
        let mut state = model.state.clone();
        state.addresses = vec!["10.0.0.2".into()];
        state.update = Some(Update { revision: "b".repeat(40) });
        model.receive(state);
        assert_eq!(model.host, "my-host");
        assert_eq!(model.invite, "draft");
        assert_eq!(
            serde_json::to_value(model.host_options().unwrap()).unwrap(),
            serde_json::json!({"mode":"host","host":"my-host","port":27843})
        );
    }
}
