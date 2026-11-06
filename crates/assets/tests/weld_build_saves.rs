//! A Website save records a Weld build as the original game's build number, and reading a
//! save with any other number fails at the skill book instead of later in play.

use serde_json::Value;
use solomon_assets::website_save::{self, CreateSaveOptions, SaveIntegrity, SaveMetadata};
use solomon_sim::modding::LuaObject;
use solomon_sim::profile::{NewGameSelection, WeldBuildId, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

const WELD_BUILD: &str = "/continuation/simulation/playerEntities/skillBooks/0/weldBuildId";

fn welded_document() -> Value {
    let mut session = GameSession::new(
        "local".into(),
        NewGameSelection {
            name: "Welder".into(),
            element: WizardElement::Fire,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let participant = session.participants.get_mut("local").unwrap();
    participant.player.grant_weld_build(WeldBuildId::FlameLash, &mut session.gameplay_rng);
    let metadata = SaveMetadata {
        integrity: SaveIntegrity::LocalOnly,
        mods: Vec::new(),
        mod_state: LuaObject::default(),
        native_source: None,
    };
    let document = website_save::create(CreateSaveOptions {
        state: &session,
        player_id: "local",
        loaded_boneyard: None,
        metadata: &metadata,
        party_rejoin_token: None,
    })
    .unwrap();
    serde_json::from_str(&document).unwrap()
}

#[test]
fn a_saved_weld_build_is_its_original_number() {
    let document = welded_document();
    assert_eq!(document.pointer(WELD_BUILD), Some(&Value::from(1003)));
    let restored = website_save::restore(&document.to_string()).unwrap();
    let book = &restored.session.participants[&restored.player_id].player.book;
    assert_eq!(book.weld_build_id, Some(WeldBuildId::FlameLash));
}

#[test]
fn a_save_with_an_unknown_weld_build_number_is_rejected() {
    for number in [999, 1010] {
        let mut document = welded_document();
        *document.pointer_mut(WELD_BUILD).unwrap() = Value::from(number);
        assert_eq!(
            website_save::restore(&document.to_string()).unwrap_err(),
            "game save player skill book 0 Weld build is invalid",
            "weldBuildId {number}"
        );
    }
}
