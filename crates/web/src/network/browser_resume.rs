use super::RESUME_KEY;
use solomon_protocol::ResumeCredentials;

/// Prefer this tab's admitted wizard, as in `save/game-resume-token.ts`. The native host
/// also persists its world across launcher exits, so remember the last profile across tabs
/// for the title's explicit Continue action. A fresh admission does not consume this token.
pub fn read_resume() -> Result<Option<ResumeCredentials>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let key = crate::storage::scoped_key(RESUME_KEY)?;
    let storage = window
        .session_storage()
        .map_err(|_| "The browser blocked game reconnection storage")?
        .ok_or("The browser has no game reconnection storage")?;
    let text =
        storage.get_item(&key).map_err(|_| "Could not read game reconnection credentials")?;
    let text = match text {
        Some(text) => Some(text),
        None => window
            .local_storage()
            .map_err(|_| "The browser blocked saved game identity storage")?
            .ok_or("The browser has no saved game identity storage")?
            .get_item(&key)
            .map_err(|_| "Could not read the saved game identity")?,
    };
    let Some(text) = text else { return Ok(None) };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| format!("Could not read game reconnection credentials: {error}"))
}

pub fn remember_resume(credentials: &ResumeCredentials) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let key = crate::storage::scoped_key(RESUME_KEY)?;
    let text = serde_json::to_string(credentials)
        .map_err(|error| format!("Could not encode game reconnection credentials: {error}"))?;
    let storage = window
        .session_storage()
        .map_err(|_| "The browser blocked game reconnection storage")?
        .ok_or("The browser has no game reconnection storage")?;
    storage
        .set_item(&key, &text)
        .map_err(|_| "Could not remember game reconnection credentials")?;
    window
        .local_storage()
        .map_err(|_| "The browser blocked saved game identity storage")?
        .ok_or("The browser has no saved game identity storage")?
        .set_item(&key, &text)
        .map_err(|_| "Could not remember the saved game identity".into())
}

/// A replaced tab may hold an obsolete credential. Remove only that pair, preserving
/// the newer credential another tab has already committed to shared local storage.
pub fn forget_rejected_resume(rejected: &ResumeCredentials) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let key = crate::storage::scoped_key(RESUME_KEY)?;
    for storage in [window.session_storage(), window.local_storage()] {
        let storage = storage
            .map_err(|_| "The browser blocked saved game identity storage")?
            .ok_or("The browser has no saved game identity storage")?;
        if let Some(text) =
            storage.get_item(&key).map_err(|_| "Could not read the saved game identity")?
        {
            let stored: ResumeCredentials = serde_json::from_str(&text)
                .map_err(|error| format!("Could not read saved game identity: {error}"))?;
            if &stored == rejected {
                storage
                    .remove_item(&key)
                    .map_err(|_| "Could not clear the rejected game identity")?;
            }
        }
    }
    Ok(())
}
