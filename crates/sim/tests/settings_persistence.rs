//! Existing offline settings retain the new source OpenChat default when loaded.
#![cfg(feature = "persistence")]

use solomon_sim::settings::{GameControlAction, GameControlBindings};

#[test]
fn older_controls_load_chat_default_and_rebound_chat_survives_reload() {
    let mut document = serde_json::to_value(GameControlBindings::default()).expect("controls JSON");
    document.as_object_mut().expect("controls object").remove("open_chat");
    let mut controls: GameControlBindings =
        serde_json::from_value(document).expect("older controls");
    assert_eq!(controls.open_chat, "KeyT");
    assert!(controls.rebind(GameControlAction::OpenChat, "KeyI"));
    assert_eq!(controls.open_inventory, "KeyT");
    let restored: GameControlBindings =
        serde_json::from_str(&serde_json::to_string(&controls).expect("saved controls"))
            .expect("restored controls");
    assert_eq!(restored, controls);
}
